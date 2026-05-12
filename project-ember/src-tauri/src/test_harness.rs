use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, WebviewWindow};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::http_util::{self, BoxBody};

const EVAL_TIMEOUT: Duration = Duration::from_secs(10);

type PendingEvals = Arc<Mutex<HashMap<String, oneshot::Sender<EvalResult>>>>;

pub struct TestHarnessState {
    pending: PendingEvals,
    pub port: u16,
}

impl TestHarnessState {
    pub fn inactive() -> Self {
        Self {
            pending: Arc::new(Mutex::new(HashMap::new())),
            port: 0,
        }
    }

    pub fn resolve(&self, id: &str, result: Option<String>, error: Option<String>) {
        let mut map = self.pending.lock().unwrap();
        if let Some(sender) = map.remove(id) {
            let _ = sender.send(EvalResult { result, error });
        }
    }
}

struct EvalResult {
    result: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct EvalRequest {
    id: String,
    js: String,
}

#[derive(Deserialize)]
struct EmitRequest {
    event: String,
    payload: serde_json::Value,
}

#[derive(Serialize)]
struct EvalResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn wrap_js(id: &str, user_js: &str) -> String {
    format!(
        r#"(async () => {{
  try {{
    const __result = await (async () => {{ {user_js} }})();
    await window.__TAURI_INTERNALS__.invoke('__test_report', {{ id: '{id}', result: JSON.stringify(__result !== undefined ? __result : null) }});
  }} catch (e) {{
    await window.__TAURI_INTERNALS__.invoke('__test_report', {{ id: '{id}', error: String(e) }});
  }}
}})();"#
    )
}

async fn handle_request(
    req: Request<Incoming>,
    pending: PendingEvals,
    app_handle: AppHandle,
    webview: WebviewWindow,
) -> Result<Response<BoxBody>, hyper::Error> {
    let (parts, body) = req.into_parts();
    let path = parts.uri.path();

    match (&parts.method, path) {
        (&Method::GET, "/test/ready") => Ok(http_util::ok_response()),

        (&Method::POST, "/test/eval") => {
            let req_body: EvalRequest = match http_util::parse_json_body(body).await {
                Ok(r) => r,
                Err(resp) => return Ok(resp),
            };

            let (sender, receiver) = oneshot::channel::<EvalResult>();

            {
                let mut map = pending.lock().unwrap();
                map.insert(req_body.id.clone(), sender);
            }

            let wrapped = wrap_js(&req_body.id, &req_body.js);
            if let Err(e) = webview.eval(&wrapped) {
                let mut map = pending.lock().unwrap();
                map.remove(&req_body.id);
                return Ok(http_util::error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("webview eval failed: {e}"),
                ));
            }

            let eval_result = match tokio::time::timeout(EVAL_TIMEOUT, receiver).await {
                Ok(Ok(r)) => r,
                Ok(Err(_)) => {
                    return Ok(http_util::error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "eval channel dropped",
                    ));
                }
                Err(_) => {
                    let mut map = pending.lock().unwrap();
                    map.remove(&req_body.id);
                    return Ok(http_util::error_response(
                        StatusCode::GATEWAY_TIMEOUT,
                        "eval timed out after 10s",
                    ));
                }
            };

            if let Some(err) = eval_result.error {
                Ok(http_util::json_response(
                    StatusCode::OK,
                    &EvalResponse {
                        result: None,
                        error: Some(err),
                    },
                ))
            } else {
                let parsed: serde_json::Value = eval_result
                    .result
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or(serde_json::Value::Null);

                Ok(http_util::json_response(
                    StatusCode::OK,
                    &EvalResponse {
                        result: Some(parsed),
                        error: None,
                    },
                ))
            }
        }

        (&Method::POST, "/test/emit") => {
            let req_body: EmitRequest = match http_util::parse_json_body(body).await {
                Ok(r) => r,
                Err(resp) => return Ok(resp),
            };

            if app_handle
                .emit(&req_body.event, req_body.payload)
                .is_err()
            {
                return Ok(http_util::error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to emit event",
                ));
            }

            Ok(http_util::ok_response())
        }

        _ => Ok(http_util::error_response(StatusCode::NOT_FOUND, "not found")),
    }
}

pub fn start(app_handle: AppHandle, webview: WebviewWindow) -> TestHarnessState {
    let pending: PendingEvals = Arc::new(Mutex::new(HashMap::new()));

    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("test_harness: failed to bind to 127.0.0.1:0");
    let port = listener
        .local_addr()
        .expect("test_harness: failed to get local addr")
        .port();
    listener
        .set_nonblocking(true)
        .expect("test_harness: failed to set nonblocking");

    let pending_clone = pending.clone();

    tauri::async_runtime::spawn(async move {
        let listener = TcpListener::from_std(listener)
            .expect("test_harness: failed to convert std listener to tokio");

        loop {
            let (stream, _addr) = match listener.accept().await {
                Ok(conn) => conn,
                Err(_) => continue,
            };

            let io = hyper_util::rt::TokioIo::new(stream);
            let pending_for_conn = pending_clone.clone();
            let handle_for_conn = app_handle.clone();
            let webview_for_conn = webview.clone();

            tokio::spawn(async move {
                let service = service_fn(move |req| {
                    handle_request(
                        req,
                        pending_for_conn.clone(),
                        handle_for_conn.clone(),
                        webview_for_conn.clone(),
                    )
                });

                let _ = http1::Builder::new().serve_connection(io, service).await;
            });
        }
    });

    TestHarnessState { pending, port }
}
