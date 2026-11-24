//! Configuration loaded exclusively from environment variables.

use anyhow::{Context, Result, bail};
use ipnet::IpNet;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub listen_addr: SocketAddr,
    pub database_path: PathBuf,
    pub trusted_proxies: Vec<IpNet>,
    pub retention_days: u32,
    pub isp_db_path: Option<PathBuf>,
    pub log_level: String,
    pub log_format: LogFormat,
    /// Largest download (bytes) a single request may ask for.
    pub max_download_bytes: u64,
    /// Largest upload (bytes) a single request may send.
    pub max_upload_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Text,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:8080".parse().unwrap(),
            database_path: PathBuf::from("/data/rustspeed.sqlite3"),
            trusted_proxies: Vec::new(),
            retention_days: 90,
            isp_db_path: None,
            log_level: "info".into(),
            log_format: LogFormat::Json,
            max_download_bytes: 1 << 30,
            max_upload_bytes: 256 << 20,
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Self::from_lookup(|k| std::env::var(k).ok())
    }

    /// Build the config from an arbitrary lookup function (testable).
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let mut c = Config::default();
        let get = |k: &str| get(k).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());

        if let Some(v) = get("RUSTSPEED_LISTEN_ADDR") {
            c.listen_addr = v
                .parse()
                .with_context(|| format!("RUSTSPEED_LISTEN_ADDR invalide: {v}"))?;
        }
        if let Some(v) = get("RUSTSPEED_DATABASE_PATH") {
            c.database_path = PathBuf::from(v);
        }
        if let Some(v) = get("RUSTSPEED_TRUSTED_PROXIES") {
            c.trusted_proxies = parse_proxies(&v)?;
        }
        if let Some(v) = get("RUSTSPEED_RETENTION_DAYS") {
            c.retention_days = v
                .parse()
                .with_context(|| format!("RUSTSPEED_RETENTION_DAYS invalide: {v}"))?;
        }
        c.isp_db_path = get("RUSTSPEED_ISP_DB_PATH").map(PathBuf::from);
        if let Some(v) = get("RUSTSPEED_LOG_LEVEL") {
            c.log_level = v;
        }
        if let Some(v) = get("RUSTSPEED_LOG_FORMAT") {
            c.log_format = match v.as_str() {
                "json" => LogFormat::Json,
                "text" => LogFormat::Text,
                other => bail!("RUSTSPEED_LOG_FORMAT invalide: {other} (json|text)"),
            };
        }
        if let Some(v) = get("RUSTSPEED_MAX_DOWNLOAD_BYTES") {
            c.max_download_bytes = v.parse().context("RUSTSPEED_MAX_DOWNLOAD_BYTES invalide")?;
        }
        if let Some(v) = get("RUSTSPEED_MAX_UPLOAD_BYTES") {
            c.max_upload_bytes = v.parse().context("RUSTSPEED_MAX_UPLOAD_BYTES invalide")?;
        }
        Ok(c)
    }
}

pub fn parse_proxies(v: &str) -> Result<Vec<IpNet>> {
    v.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<IpNet>()
                .or_else(|_| s.parse::<IpAddr>().map(IpNet::from))
                .with_context(|| format!("RUSTSPEED_TRUSTED_PROXIES: entrée invalide '{s}'"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn cfg(pairs: &[(&str, &str)]) -> Result<Config> {
        let m: HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        Config::from_lookup(|k| m.get(k).cloned())
    }

    #[test]
    fn defaults() {
        let c = cfg(&[]).unwrap();
        assert_eq!(c.listen_addr.port(), 8080);
        assert_eq!(c.retention_days, 90);
        assert!(c.trusted_proxies.is_empty());
    }

    #[test]
    fn parses_proxies_and_ips() {
        let c = cfg(&[("RUSTSPEED_TRUSTED_PROXIES", "10.0.0.0/8, 192.168.1.1")]).unwrap();
        assert_eq!(c.trusted_proxies.len(), 2);
    }

    #[test]
    fn rejects_invalid_values() {
        assert!(cfg(&[("RUSTSPEED_RETENTION_DAYS", "abc")]).is_err());
        assert!(cfg(&[("RUSTSPEED_LISTEN_ADDR", "nope")]).is_err());
        assert!(cfg(&[("RUSTSPEED_TRUSTED_PROXIES", "1.2.3.400")]).is_err());
    }
}
