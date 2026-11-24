//! Router, shared state and non-measurement handlers.

use crate::client_info::{self, ClientInfo, IspDb};
use crate::config::Config;
use crate::{db, measure};
use axum::body::Bytes;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use hickory_resolver::TokioResolver;
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::json;
use sqlx::SqlitePool;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub pool: SqlitePool,
    pub random: Bytes,
    pub resolver: Option<Arc<TokioResolver>>,
    pub isp_db: Option<Arc<IspDb>>,
}

#[derive(RustEmbed)]
#[folder = "web/dist/"]
#[allow_missing = true]
struct Assets;

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/measure/download", get(measure::download))
        .route(
            "/measure/upload",
            post(measure::upload).layer(DefaultBodyLimit::disable()),
        )
        .route("/measure/ws", get(measure::ws))
        .route("/client-info", get(client_info_handler))
        .route("/results", post(create_result).get(list_results))
        .route("/results/{token}", get(get_result))
        .fallback(|| async { ApiError::new(StatusCode::NOT_FOUND, "route inconnue") });

    Router::new()
        .nest("/api/v1", api)
        .route("/healthz", get(healthz))
        .fallback(static_handler)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// ---------- errors ----------

pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }
    fn internal(e: impl std::fmt::Display) -> Self {
        tracing::error!(error = %e, "internal error");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "erreur interne")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

// ---------- handlers ----------

async fn healthz(State(st): State<AppState>) -> Response {
    match sqlx::query("SELECT 1").execute(&st.pool).await {
        Ok(_) => (StatusCode::OK, "ok").into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "healthcheck: database unavailable");
            (StatusCode::SERVICE_UNAVAILABLE, "database unavailable").into_response()
        }
    }
}

fn effective_ip(st: &AppState, peer: SocketAddr, headers: &HeaderMap) -> IpAddr {
    let xff = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok());
    client_info::client_ip(peer.ip(), xff, &st.config.trusted_proxies)
}

async fn resolve(st: &AppState, ip: IpAddr) -> ClientInfo {
    client_info::lookup(ip, st.resolver.as_deref(), st.isp_db.as_deref()).await
}

async fn client_info_handler(
    State(st): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Json<ClientInfo> {
    let ip = effective_ip(&st, peer, &headers);
    Json(resolve(&st, ip).await)
}

async fn create_result(
    State(st): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(body): Json<db::NewResult>,
) -> Result<(StatusCode, Json<db::StoredResult>), ApiError> {
    body.validate().map_err(ApiError::bad_request)?;
    let ip = effective_ip(&st, peer, &headers);
    let info = resolve(&st, ip).await;
    let ua = headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok());
    let saved = db::insert(
        &st.pool,
        &body,
        &info.ip,
        info.isp.as_deref(),
        ua,
        st.config.retention_days,
    )
    .await
    .map_err(ApiError::internal)?;
    tracing::info!(token = %saved.share_token, download = body.download_mbps, upload = body.upload_mbps, "result saved");
    Ok((StatusCode::CREATED, Json(saved)))
}

async fn get_result(State(st): State<AppState>, Path(token): Path<String>) -> Result<Json<db::StoredResult>, ApiError> {
    if !db::is_valid_token(&token) {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "résultat introuvable"));
    }
    db::get(&st.pool, &token)
        .await
        .map_err(ApiError::internal)?
        .map(Json)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "résultat introuvable ou expiré"))
}

#[derive(Deserialize)]
struct ListParams {
    limit: Option<i64>,
}

async fn list_results(
    State(st): State<AppState>,
    Query(p): Query<ListParams>,
) -> Result<Json<Vec<db::StoredResult>>, ApiError> {
    let limit = p.limit.unwrap_or(20).clamp(1, 200);
    db::recent(&st.pool, limit).await.map(Json).map_err(ApiError::internal)
}

/// Serve the embedded Svelte build. Unknown paths (e.g. `/r/<token>`) get
/// `index.html` so the client-side router can take over.
async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let (file, path) = match Assets::get(path).filter(|_| !path.is_empty()) {
        Some(f) => (f, path),
        None => match Assets::get("index.html") {
            Some(f) => (f, "index.html"),
            None => return (StatusCode::NOT_FOUND, "UI non embarquée (web/dist absent au build)").into_response(),
        },
    };
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let cache = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).unwrap()),
            (header::CACHE_CONTROL, HeaderValue::from_static(cache)),
        ],
        file.data,
    )
        .into_response()
}
