//! Measurement endpoints: download stream, upload sink, WebSocket echo.

use crate::app::{ApiError, AppState};
use axum::Json;
use axum::body::{Body, Bytes};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Size of the random block repeated to build download bodies. Random data is
/// incompressible; the period (4 MiB) is far larger than any compressor window.
pub const RANDOM_BLOCK: usize = 4 << 20;
const CHUNK: usize = 256 << 10;

pub fn random_block() -> Bytes {
    let mut v = vec![0u8; RANDOM_BLOCK];
    getrandom::fill(&mut v).expect("OS RNG unavailable");
    Bytes::from(v)
}

#[derive(Deserialize)]
pub struct DownloadParams {
    bytes: Option<u64>,
}

pub async fn download(State(st): State<AppState>, Query(p): Query<DownloadParams>) -> Result<Response, ApiError> {
    let total = p.bytes.unwrap_or(25 << 20);
    if total == 0 || total > st.config.max_download_bytes {
        return Err(ApiError::bad_request(format!(
            "bytes doit être compris entre 1 et {}",
            st.config.max_download_bytes
        )));
    }
    let block = st.random.clone();
    let stream = futures_util::stream::unfold(0u64, move |sent| {
        let block = block.clone();
        async move {
            if sent >= total {
                return None;
            }
            let off = (sent % RANDOM_BLOCK as u64) as usize;
            let len = (CHUNK as u64).min(total - sent).min((RANDOM_BLOCK - off) as u64) as usize;
            Some((Ok::<_, std::io::Error>(block.slice(off..off + len)), sent + len as u64))
        }
    });
    let mut res = Body::from_stream(stream).into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    h.insert(header::CONTENT_LENGTH, HeaderValue::from(total));
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-transform"),
    );
    h.insert(header::CONTENT_ENCODING, HeaderValue::from_static("identity"));
    Ok(res)
}

#[derive(Serialize)]
pub struct UploadReport {
    pub bytes: u64,
    pub server_duration_ms: f64,
}

pub async fn upload(State(st): State<AppState>, body: Body) -> Result<Json<UploadReport>, ApiError> {
    let max = st.config.max_upload_bytes;
    let mut stream = body.into_data_stream();
    let mut bytes = 0u64;
    let mut start: Option<Instant> = None;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ApiError::bad_request(format!("corps illisible: {e}")))?;
        start.get_or_insert_with(Instant::now);
        bytes += chunk.len() as u64;
        if bytes > max {
            return Err(ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("upload limité à {max} octets"),
            ));
        }
    }
    let ms = start.map(|s| s.elapsed().as_secs_f64() * 1000.0).unwrap_or(0.0);
    Ok(Json(UploadReport {
        bytes,
        server_duration_ms: ms,
    }))
}

pub async fn ws(ws: WebSocketUpgrade) -> Response {
    ws.max_message_size(4096).on_upgrade(echo)
}

/// Echo every frame back immediately; the client timestamps and computes RTT.
async fn echo(mut socket: WebSocket) {
    let idle = Duration::from_secs(60);
    while let Ok(Some(Ok(msg))) = tokio::time::timeout(idle, socket.recv()).await {
        let reply = match msg {
            Message::Text(t) => Message::Text(t),
            Message::Binary(b) => Message::Binary(b),
            Message::Close(_) => break,
            _ => continue,
        };
        if socket.send(reply).await.is_err() {
            break;
        }
    }
}
