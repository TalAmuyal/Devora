//! The app's local IPC server: an HTTP server on `127.0.0.1` through which tools inside its terminals reach the UI.
//!
//! Terminals find it through `DEVORA_IPC_PORT` (see `devora_terminal`).
//! Its one route, `POST /preview/open` (sent by `debi preview`), is handed to the UI through a callback.
//! The server runs on its own thread, so it needs no async runtime from the UI.

use std::io;
use std::sync::Arc;

use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

/// A request to render the file at `path` in a preview pane beside the terminal `pty_id`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewOpen {
    /// The terminal's `DEVORA_PTY_ID`.
    pub pty_id: u32,
    pub path: String,
    /// Open another pane instead of replacing the current preview.
    pub stack: bool,
}

/// How the UI handles the server's requests and failures.
pub struct IpcHandlers {
    /// Handles `POST /preview/open`; an error fails the request with a 500.
    pub on_preview_open: Box<dyn Fn(PreviewOpen) -> Result<(), String> + Send + Sync>,
    /// Reports a failure that no request can carry back, such as a failed accept.
    pub on_error: Box<dyn Fn(&str) + Send + Sync>,
}

pub struct IpcServer {
    port: u16,
}

impl IpcServer {
    pub fn port(&self) -> u16 {
        self.port
    }
}

/// Binds a free port on `127.0.0.1` and serves it on a new thread for the rest of the process.
pub fn start(handlers: IpcHandlers) -> io::Result<IpcServer> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()?;
    let handlers = Arc::new(handlers);

    std::thread::Builder::new()
        .name("devora-ipc".into())
        .spawn(move || runtime.block_on(serve(listener, handlers)))?;

    Ok(IpcServer { port })
}

async fn serve(listener: std::net::TcpListener, handlers: Arc<IpcHandlers>) {
    let listener = match TcpListener::from_std(listener) {
        Ok(listener) => listener,
        Err(e) => {
            (handlers.on_error)(&format!("IPC server failed to start: {e}"));
            return;
        }
    };

    loop {
        let stream = match listener.accept().await {
            Ok((stream, _addr)) => stream,
            Err(e) => {
                (handlers.on_error)(&format!("IPC server accept failed: {e}"));
                continue;
            }
        };

        let io = hyper_util::rt::TokioIo::new(stream);
        let handlers = handlers.clone();
        tokio::spawn(async move {
            let service = service_fn(move |req| handle(req, handlers.clone()));
            let _ = http1::Builder::new().serve_connection(io, service).await;
        });
    }
}

async fn handle(
    req: Request<Incoming>,
    handlers: Arc<IpcHandlers>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let (parts, body) = req.into_parts();

    match (&parts.method, parts.uri.path()) {
        // Fire-and-forget: render a file in a preview pane beside the terminal.
        (&Method::POST, "/preview/open") => {
            let request: PreviewOpen = match parse_json_body(body).await {
                Ok(request) => request,
                Err(message) => return Ok(error_response(StatusCode::BAD_REQUEST, &message)),
            };
            Ok(match (handlers.on_preview_open)(request) {
                Ok(()) => json_response(StatusCode::OK, &serde_json::json!({ "ok": true })),
                Err(message) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &message),
            })
        }
        _ => {
            // Read the body even so: closing the connection with it unread makes the kernel reset it, which can discard the response before the client reads it.
            let _ = body.collect().await;
            Ok(error_response(StatusCode::NOT_FOUND, "not found"))
        }
    }
}

async fn parse_json_body<T: serde::de::DeserializeOwned>(body: Incoming) -> Result<T, String> {
    let bytes = body
        .collect()
        .await
        .map_err(|_| "failed to read body".to_string())?
        .to_bytes();
    serde_json::from_slice(&bytes).map_err(|e| format!("invalid json: {e}"))
}

fn json_response(status: StatusCode, body: &serde_json::Value) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(body.to_string())))
        .expect("a status and a static header always make a valid response")
}

fn error_response(status: StatusCode, message: &str) -> Response<Full<Bytes>> {
    json_response(status, &serde_json::json!({ "error": message }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::sync::Mutex;

    /// Starts a server whose preview handler records each request and answers with `reply`.
    fn server(reply: Result<(), String>) -> (IpcServer, Arc<Mutex<Vec<PreviewOpen>>>) {
        let opened = Arc::new(Mutex::new(Vec::new()));
        let sink = opened.clone();
        let server = start(IpcHandlers {
            on_preview_open: Box::new(move |request| {
                sink.lock().unwrap().push(request);
                reply.clone()
            }),
            on_error: Box::new(|message| panic!("unexpected IPC error: {message}")),
        })
        .unwrap();
        (server, opened)
    }

    /// Sends one request and returns the status code and body.
    fn send(server: &IpcServer, method: &str, path: &str, body: &str) -> (u16, String) {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", server.port())).unwrap();
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let status = response[9..12].parse().unwrap();
        let body = response.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
        (status, body)
    }

    #[test]
    fn preview_open_reaches_the_ui() {
        let (server, opened) = server(Ok(()));

        let (status, body) = send(
            &server,
            "POST",
            "/preview/open",
            r#"{"ptyId": 3, "path": "/tmp/notes.md", "stack": true}"#,
        );

        assert_eq!((status, body.as_str()), (200, r#"{"ok":true}"#));
        assert_eq!(
            *opened.lock().unwrap(),
            [PreviewOpen {
                pty_id: 3,
                path: "/tmp/notes.md".into(),
                stack: true,
            }]
        );
    }

    #[test]
    fn a_malformed_preview_request_is_rejected_before_the_ui() {
        let (server, opened) = server(Ok(()));

        let (status, body) = send(&server, "POST", "/preview/open", r#"{"ptyId": 3}"#);

        assert_eq!(status, 400);
        assert!(body.contains("invalid json"), "{body}");
        assert!(opened.lock().unwrap().is_empty());
    }

    #[test]
    fn a_ui_failure_fails_the_request() {
        let (server, _) = server(Err("failed to emit event".into()));

        let (status, body) = send(
            &server,
            "POST",
            "/preview/open",
            r#"{"ptyId": 3, "path": "/tmp/notes.md", "stack": false}"#,
        );

        assert_eq!(
            (status, body.as_str()),
            (500, r#"{"error":"failed to emit event"}"#)
        );
    }

    #[test]
    fn other_routes_are_not_found() {
        let (server, opened) = server(Ok(()));

        assert_eq!(send(&server, "GET", "/preview/open", "").0, 404);
        assert_eq!(send(&server, "POST", "/elsewhere", "{}").0, 404);
        assert!(opened.lock().unwrap().is_empty());
    }

    #[test]
    fn the_ui_serializes_a_preview_in_the_request_shape() {
        let request = PreviewOpen {
            pty_id: 3,
            path: "/tmp/notes.md".into(),
            stack: false,
        };

        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            serde_json::json!({ "ptyId": 3, "path": "/tmp/notes.md", "stack": false })
        );
    }
}
