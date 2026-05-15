use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::{Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub type BoxBody = Full<Bytes>;

pub fn json_response(status: StatusCode, body: &impl Serialize) -> Response<BoxBody> {
    let json = serde_json::to_string(body).unwrap_or_else(|_| r#"{"error":"serialize"}"#.into());
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(json)))
        .unwrap()
}

pub fn error_response(status: StatusCode, message: &str) -> Response<BoxBody> {
    let body = serde_json::json!({"error": message});
    json_response(status, &body)
}

pub fn ok_response() -> Response<BoxBody> {
    Response::builder()
        .status(StatusCode::OK)
        .body(Full::new(Bytes::new()))
        .unwrap()
}

pub async fn parse_json_body<T: DeserializeOwned>(body: Incoming) -> Result<T, Response<BoxBody>> {
    let body_bytes = body
        .collect()
        .await
        .map(|collected| collected.to_bytes())
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "failed to read body"))?;

    serde_json::from_slice(&body_bytes)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("invalid json: {e}")))
}
