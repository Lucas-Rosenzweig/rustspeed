use rustspeed::config::{Config, LogFormat};
use std::net::SocketAddr;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        return healthcheck().await;
    }

    let config = Config::from_env()?;
    let filter = EnvFilter::try_new(&config.log_level)?;
    match config.log_format {
        LogFormat::Json => tracing_subscriber::fmt().json().with_env_filter(filter).init(),
        LogFormat::Text => tracing_subscriber::fmt().with_env_filter(filter).init(),
    }

    let addr = config.listen_addr;
    let retention = config.retention_days;
    tracing::info!(
        %addr,
        db = %config.database_path.display(),
        retention_days = retention,
        trusted_proxies = config.trusted_proxies.len(),
        "starting rustspeed {}",
        env!("CARGO_PKG_VERSION")
    );
    let state = rustspeed::build_state(config).await?;

    if retention > 0 {
        let pool = state.pool.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(3600));
            loop {
                tick.tick().await;
                match rustspeed::db::purge_expired(&pool).await {
                    Ok(0) => {}
                    Ok(n) => tracing::info!(purged = n, "expired results purged"),
                    Err(e) => tracing::error!(error = %e, "purge failed"),
                }
            }
        });
    }

    let app = rustspeed::app::router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    let ctrl_c = tokio::signal::ctrl_c();
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("signal handler");
    tokio::select! {
        _ = ctrl_c => {},
        _ = term.recv() => {},
    }
    tracing::info!("shutting down");
}

/// `rustspeed healthcheck`: used by the Docker HEALTHCHECK (distroless image has no curl).
async fn healthcheck() -> anyhow::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let addr: SocketAddr = std::env::var("RUSTSPEED_LISTEN_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".into())
        .parse()?;
    let target = SocketAddr::from(([127, 0, 0, 1], addr.port()));
    let mut s = tokio::time::timeout(Duration::from_secs(3), tokio::net::TcpStream::connect(target)).await??;
    s.write_all(b"GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n").await?;
    let mut buf = Vec::new();
    tokio::time::timeout(Duration::from_secs(3), s.read_to_end(&mut buf)).await??;
    let head = String::from_utf8_lossy(&buf[..buf.len().min(16)]).to_string();
    if head.contains(" 200") {
        Ok(())
    } else {
        anyhow::bail!("unhealthy: {head}")
    }
}
