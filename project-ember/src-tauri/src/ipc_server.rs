use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::net::TcpListener;

use crate::http_util::{self, BoxBody};

pub struct IpcState {
    pub port: u16,
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
    app_handle: AppHandle,
) -> Result<Response<BoxBody>, hyper::Error> {
    let (parts, body) = req.into_parts();
    let path = parts.uri.path();

    match (&parts.method, path) {
        // Fire-and-forget: render a file in a preview pane beside the terminal.
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
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("ipc_server: failed to bind to 127.0.0.1:0");
    let port = listener
        .local_addr()
        .expect("ipc_server: failed to get local addr")
        .port();
    listener
        .set_nonblocking(true)
        .expect("ipc_server: failed to set nonblocking");

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
            let handle_for_conn = app_handle.clone();

            tokio::spawn(async move {
                let service = service_fn(move |req| {
                    handle_request(req, handle_for_conn.clone())
                });

                let _ = http1::Builder::new().serve_connection(io, service).await;
            });
        }
    });

    IpcState { port }
}
