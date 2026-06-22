use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::http_util::{self, BoxBody};

pub struct IpcState {
    pending: Arc<Mutex<HashMap<u32, oneshot::Sender<String>>>>,
    pub port: u16,
}

impl IpcState {
    pub fn resolve(&self, pty_id: u32, result: String) {
        let mut map = self.pending.lock().unwrap();
        if let Some(sender) = map.remove(&pty_id) {
            let _ = sender.send(result);
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CritOpenRequest {
    pty_id: u32,
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CritDoneRequest {
    pty_id: u32,
    reason: String,
}

#[derive(Serialize)]
struct CritOpenResponse {
    result: String,
}

#[derive(Serialize)]
struct CritDoneResponse {
    ok: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CritOpenOverlayPayload {
    pty_id: u32,
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewOpenRequest {
    pty_id: u32,
    path: String,
    stack: bool,
}

#[derive(Serialize)]
struct PreviewOpenResponse {
    ok: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PreviewOpenPayload {
    pty_id: u32,
    path: String,
    stack: bool,
}

async fn handle_request(
    req: Request<Incoming>,
    pending: Arc<Mutex<HashMap<u32, oneshot::Sender<String>>>>,
    app_handle: AppHandle,
) -> Result<Response<BoxBody>, hyper::Error> {
    let (parts, body) = req.into_parts();
    let path = parts.uri.path();

    match (&parts.method, path) {
        (&Method::POST, "/crit/open") => {
            let req_body: CritOpenRequest = match http_util::parse_json_body(body).await {
                Ok(r) => r,
                Err(resp) => return Ok(resp),
            };

            let (sender, receiver) = oneshot::channel::<String>();

            {
                let mut map = pending.lock().unwrap();
                // If there is already a pending request for this pty, resolve it as
                // "dismissed" before replacing it.
                if let Some(old_sender) = map.remove(&req_body.pty_id) {
                    let _ = old_sender.send("dismissed".to_string());
                }
                map.insert(req_body.pty_id, sender);
            }

            let payload = CritOpenOverlayPayload {
                pty_id: req_body.pty_id,
                url: req_body.url,
            };

            if app_handle.emit("crit-open-overlay", payload).is_err() {
                let mut map = pending.lock().unwrap();
                map.remove(&req_body.pty_id);
                return Ok(http_util::error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to emit event",
                ));
            }

            // Block until the overlay is resolved (submitted/dismissed/crashed).
            let result = match receiver.await {
                Ok(r) => r,
                Err(_) => "dismissed".to_string(),
            };

            Ok(http_util::json_response(StatusCode::OK, &CritOpenResponse { result }))
        }

        (&Method::POST, "/crit/done") => {
            let req_body: CritDoneRequest = match http_util::parse_json_body(body).await {
                Ok(r) => r,
                Err(resp) => return Ok(resp),
            };

            {
                let mut map = pending.lock().unwrap();
                if let Some(sender) = map.remove(&req_body.pty_id) {
                    let _ = sender.send(req_body.reason.clone());
                }
            }

            let close_payload = CritOpenOverlayPayload {
                pty_id: req_body.pty_id,
                url: String::new(),
            };
            if app_handle.emit("crit-close-overlay", close_payload).is_err() {
                crate::logging::report_error(
                    &app_handle,
                    &format!("failed to emit crit-close-overlay for pty {}", req_body.pty_id),
                );
            }

            Ok(http_util::json_response(StatusCode::OK, &CritDoneResponse { ok: true }))
        }

        // Fire-and-forget: render a file in a preview pane beside the terminal.
        // Unlike /crit/open this does not block — the pane is closed in-app via its × button.
        (&Method::POST, "/preview/open") => {
            let req_body: PreviewOpenRequest = match http_util::parse_json_body(body).await {
                Ok(r) => r,
                Err(resp) => return Ok(resp),
            };

            let payload = PreviewOpenPayload {
                pty_id: req_body.pty_id,
                path: req_body.path,
                stack: req_body.stack,
            };

            if app_handle.emit("preview-open", payload).is_err() {
                return Ok(http_util::error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to emit event",
                ));
            }

            Ok(http_util::json_response(StatusCode::OK, &PreviewOpenResponse { ok: true }))
        }

        _ => {
            Ok(http_util::error_response(StatusCode::NOT_FOUND, "not found"))
        }
    }
}

pub fn start(app_handle: AppHandle) -> IpcState {
    let pending: Arc<Mutex<HashMap<u32, oneshot::Sender<String>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("ipc_server: failed to bind to 127.0.0.1:0");
    let port = listener
        .local_addr()
        .expect("ipc_server: failed to get local addr")
        .port();
    listener
        .set_nonblocking(true)
        .expect("ipc_server: failed to set nonblocking");

    let pending_clone = pending.clone();

    tauri::async_runtime::spawn(async move {
        let listener = TcpListener::from_std(listener)
            .expect("ipc_server: failed to convert std listener to tokio");

        loop {
            let (stream, _addr) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    crate::logging::report_error(
                        &app_handle,
                        &format!("IPC server accept failed: {e}"),
                    );
                    continue;
                }
            };

            let io = hyper_util::rt::TokioIo::new(stream);
            let pending_for_conn = pending_clone.clone();
            let handle_for_conn = app_handle.clone();

            tokio::spawn(async move {
                let service = service_fn(move |req| {
                    handle_request(req, pending_for_conn.clone(), handle_for_conn.clone())
                });

                let _ = http1::Builder::new().serve_connection(io, service).await;
            });
        }
    });

    IpcState { pending, port }
}
