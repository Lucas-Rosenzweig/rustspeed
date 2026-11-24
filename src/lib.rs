pub mod app;
pub mod client_info;
pub mod config;
pub mod db;
pub mod measure;

use std::sync::Arc;

/// Build the application state from a config (DB, random block, resolver, ISP DB).
pub async fn build_state(config: config::Config) -> anyhow::Result<app::AppState> {
    let pool = db::connect(&config.database_path).await?;
    let resolver = match hickory_resolver::TokioResolver::builder_tokio() {
        Ok(b) => Some(Arc::new(b.build()?)),
        Err(e) => {
            tracing::warn!(error = %e, "reverse DNS disabled: no system resolver config");
            None
        }
    };
    let isp_db = match &config.isp_db_path {
        Some(p) => {
            let db = client_info::IspDb::load(p)?;
            tracing::info!(path = %p.display(), ranges = db.len(), "ISP database loaded");
            Some(Arc::new(db))
        }
        None => None,
    };
    Ok(app::AppState {
        config: Arc::new(config),
        pool,
        random: measure::random_block(),
        resolver,
        isp_db,
    })
}
