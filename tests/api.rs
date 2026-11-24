//! End-to-end tests against a real listening server.

use rustspeed::config::{Config, parse_proxies};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn spawn(mut config: Config) -> (SocketAddr, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    config.database_path = dir.path().join("db.sqlite3");
    let state = rustspeed::build_state(config).await.unwrap();
    let app = rustspeed::app::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap()
    });
    (addr, dir)
}

/// Minimal HTTP/1.0 client: returns (status, headers, body).
async fn http(addr: SocketAddr, req: &str, body: &[u8]) -> (u16, String, Vec<u8>) {
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    s.write_all(req.as_bytes()).await.unwrap();
    s.write_all(body).await.unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).await.unwrap();
    let split = buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&buf[..split]).to_string();
    let status = head[9..12].parse().unwrap();
    (status, head, buf[split + 4..].to_vec())
}

fn get(path: &str, extra: &str) -> String {
    format!("GET {path} HTTP/1.0\r\nHost: t\r\n{extra}\r\n")
}

fn post(path: &str, ctype: &str, len: usize, extra: &str) -> String {
    format!("POST {path} HTTP/1.0\r\nHost: t\r\nContent-Type: {ctype}\r\nContent-Length: {len}\r\n{extra}\r\n")
}

#[tokio::test]
async fn healthz_ok() {
    let (addr, _d) = spawn(Config::default()).await;
    let (st, _, body) = http(addr, &get("/healthz", ""), b"").await;
    assert_eq!(st, 200);
    assert_eq!(body, b"ok");
}

#[tokio::test]
async fn download_is_exact_and_incompressible() {
    let (addr, _d) = spawn(Config::default()).await;
    let n = 9 * 1024 * 1024 + 17;
    let (st, head, body) = http(addr, &get(&format!("/api/v1/measure/download?bytes={n}"), ""), b"").await;
    assert_eq!(st, 200);
    assert!(head.to_lowercase().contains("cache-control: no-store"));
    assert_eq!(body.len(), n);
    // Random data: byte histogram should be roughly flat (no byte > 2x expected).
    let mut hist = [0usize; 256];
    body.iter().for_each(|b| hist[*b as usize] += 1);
    let expected = n / 256;
    assert!(hist.iter().all(|&c| c < expected * 2 && c > expected / 2));
}

#[tokio::test]
async fn download_rejects_out_of_range() {
    let (addr, _d) = spawn(Config::default()).await;
    let (st, _, _) = http(addr, &get("/api/v1/measure/download?bytes=0", ""), b"").await;
    assert_eq!(st, 400);
    let (st, _, _) = http(addr, &get("/api/v1/measure/download?bytes=99999999999999", ""), b"").await;
    assert_eq!(st, 400);
}

#[tokio::test]
async fn upload_counts_bytes() {
    let (addr, _d) = spawn(Config::default()).await;
    let data = vec![7u8; 3_000_000];
    let (st, _, body) = http(
        addr,
        &post("/api/v1/measure/upload", "application/octet-stream", data.len(), ""),
        &data,
    )
    .await;
    assert_eq!(st, 200);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["bytes"], 3_000_000);
}

#[tokio::test]
async fn upload_limit_enforced() {
    let cfg = Config {
        max_upload_bytes: 1000,
        ..Config::default()
    };
    let (addr, _d) = spawn(cfg).await;
    let data = vec![0u8; 5000];
    let (st, _, _) = http(
        addr,
        &post("/api/v1/measure/upload", "application/octet-stream", data.len(), ""),
        &data,
    )
    .await;
    assert_eq!(st, 413);
}

#[tokio::test]
async fn client_info_respects_trusted_proxies() {
    // Untrusted: header ignored.
    let (addr, _d) = spawn(Config::default()).await;
    let (_, _, body) = http(addr, &get("/api/v1/client-info", "X-Forwarded-For: 9.9.9.9\r\n"), b"").await;
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["ip"], "127.0.0.1");

    // Trusted loopback: header honoured.
    let cfg = Config {
        trusted_proxies: parse_proxies("127.0.0.0/8").unwrap(),
        ..Config::default()
    };
    let (addr, _d2) = spawn(cfg).await;
    let (_, _, body) = http(
        addr,
        &get("/api/v1/client-info", "X-Forwarded-For: 6.6.6.6, 100.80.1.2\r\n"),
        b"",
    )
    .await;
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["ip"], "100.80.1.2");
}

#[tokio::test]
async fn results_roundtrip_and_share() {
    let (addr, _d) = spawn(Config::default()).await;
    let payload = br#"{"download_mbps":912.4,"upload_mbps":388.1,"ping_ms":2.4,"jitter_ms":0.3}"#;
    let (st, _, body) = http(
        addr,
        &post("/api/v1/results", "application/json", payload.len(), ""),
        payload,
    )
    .await;
    assert_eq!(st, 201);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = v["share_token"].as_str().unwrap().to_string();
    assert_eq!(token.len(), 22);

    let (st, _, body) = http(addr, &get(&format!("/api/v1/results/{token}"), ""), b"").await;
    assert_eq!(st, 200);
    let r: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(r["download_mbps"], 912.4);
    assert_eq!(r["client_ip"], "127.0.0.1");

    let (st, _, _) = http(addr, &get("/api/v1/results/AAAAAAAAAAAAAAAAAAAAAA", ""), b"").await;
    assert_eq!(st, 404);
    let (st, _, _) = http(addr, &get("/api/v1/results/../../etc", ""), b"").await;
    assert_eq!(st, 404);

    let (st, _, body) = http(addr, &get("/api/v1/results?limit=5", ""), b"").await;
    assert_eq!(st, 200);
    assert_eq!(
        serde_json::from_slice::<Vec<serde_json::Value>>(&body).unwrap().len(),
        1
    );
}

#[tokio::test]
async fn results_validation() {
    let (addr, _d) = spawn(Config::default()).await;
    let payload = br#"{"download_mbps":-5,"upload_mbps":1,"ping_ms":1,"jitter_ms":1}"#;
    let (st, _, _) = http(
        addr,
        &post("/api/v1/results", "application/json", payload.len(), ""),
        payload,
    )
    .await;
    assert_eq!(st, 400);
}

#[tokio::test]
async fn no_librespeed_endpoints() {
    let (addr, _d) = spawn(Config::default()).await;
    for p in ["/api/v1/garbage", "/api/v1/empty", "/api/v1/getIP"] {
        let (st, _, _) = http(addr, &get(p, ""), b"").await;
        assert_eq!(st, 404, "{p}");
    }
}

#[tokio::test]
async fn websocket_echo() {
    use futures_util::{SinkExt, StreamExt};
    let (addr, _d) = spawn(Config::default()).await;
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/api/v1/measure/ws"))
        .await
        .unwrap();
    for i in 0..5 {
        let msg = format!("{{\"seq\":{i}}}");
        ws.send(tokio_tungstenite::tungstenite::Message::Text(msg.clone().into()))
            .await
            .unwrap();
        let back = ws.next().await.unwrap().unwrap();
        assert_eq!(back.into_text().unwrap().as_str(), msg);
    }
}
