//! SQLite persistence for test results.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

pub async fn connect(path: &Path) -> anyhow::Result<SqlitePool> {
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir)?;
    }
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(5));
    let pool = SqlitePoolOptions::new().max_connections(4).connect_with(opts).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// Measurements submitted by the browser at the end of a test.
#[derive(Debug, Deserialize)]
pub struct NewResult {
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub ping_ms: f64,
    pub jitter_ms: f64,
}

impl NewResult {
    pub fn validate(&self) -> Result<(), &'static str> {
        let ok = |v: f64, max: f64| v.is_finite() && (0.0..=max).contains(&v);
        if !ok(self.download_mbps, 1_000_000.0) || !ok(self.upload_mbps, 1_000_000.0) {
            return Err("débit invalide");
        }
        if !ok(self.ping_ms, 60_000.0) || !ok(self.jitter_ms, 60_000.0) {
            return Err("latence invalide");
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, sqlx::FromRow, PartialEq)]
pub struct StoredResult {
    pub share_token: String,
    pub created_at: String,
    pub client_ip: String,
    pub client_isp: Option<String>,
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub ping_ms: f64,
    pub jitter_ms: f64,
    pub user_agent: Option<String>,
    pub expires_at: Option<String>,
}

/// 128 bits from the OS CSPRNG, URL-safe base64 without padding (22 chars).
pub fn new_share_token() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("OS RNG unavailable");
    b64url(&b)
}

fn b64url(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, &b)| acc | (b as u32) << (16 - 8 * i));
        let chars = chunk.len() + 1;
        for i in 0..chars {
            out.push(A[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

pub fn is_valid_token(t: &str) -> bool {
    t.len() == 22 && t.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

fn ts(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub async fn insert(
    pool: &SqlitePool,
    r: &NewResult,
    client_ip: &str,
    client_isp: Option<&str>,
    user_agent: Option<&str>,
    retention_days: u32,
) -> sqlx::Result<StoredResult> {
    let now = Utc::now();
    let expires = (retention_days > 0).then(|| ts(now + Duration::days(retention_days as i64)));
    let id = new_share_token();
    let token = new_share_token();
    let ua = user_agent.map(|u| u.chars().take(512).collect::<String>());
    sqlx::query(
        "INSERT INTO results (id, share_token, created_at, client_ip, client_isp, download_mbps, upload_mbps, ping_ms, jitter_ms, user_agent, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&token)
    .bind(ts(now))
    .bind(client_ip)
    .bind(client_isp)
    .bind(r.download_mbps)
    .bind(r.upload_mbps)
    .bind(r.ping_ms)
    .bind(r.jitter_ms)
    .bind(&ua)
    .bind(&expires)
    .execute(pool)
    .await?;
    get(pool, &token).await.map(|o| o.expect("row just inserted"))
}

pub async fn get(pool: &SqlitePool, token: &str) -> sqlx::Result<Option<StoredResult>> {
    sqlx::query_as(
        "SELECT share_token, created_at, client_ip, client_isp, download_mbps, upload_mbps, ping_ms, jitter_ms, user_agent, expires_at FROM results WHERE share_token = ? AND (expires_at IS NULL OR expires_at > ?)",
    )
    .bind(token)
    .bind(ts(Utc::now()))
    .fetch_optional(pool)
    .await
}

pub async fn recent(pool: &SqlitePool, limit: i64) -> sqlx::Result<Vec<StoredResult>> {
    sqlx::query_as(
        "SELECT share_token, created_at, client_ip, client_isp, download_mbps, upload_mbps, ping_ms, jitter_ms, user_agent, expires_at FROM results WHERE expires_at IS NULL OR expires_at > ? ORDER BY created_at DESC LIMIT ?",
    )
    .bind(ts(Utc::now()))
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn purge_expired(pool: &SqlitePool) -> sqlx::Result<u64> {
    let r = sqlx::query("DELETE FROM results WHERE expires_at IS NOT NULL AND expires_at <= ?")
        .bind(ts(Utc::now()))
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_unique_and_well_formed() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..10_000 {
            let t = new_share_token();
            assert!(is_valid_token(&t), "{t}");
            assert!(seen.insert(t));
        }
    }

    #[test]
    fn b64_known_vector() {
        assert_eq!(b64url(b"foobar"), "Zm9vYmFy");
        assert_eq!(b64url(b"fo"), "Zm8");
        assert_eq!(b64url(&[0xfb, 0xff]), "-_8");
    }

    #[test]
    fn validation() {
        let mut r = NewResult {
            download_mbps: 900.0,
            upload_mbps: 300.0,
            ping_ms: 3.2,
            jitter_ms: 0.4,
        };
        assert!(r.validate().is_ok());
        r.download_mbps = f64::NAN;
        assert!(r.validate().is_err());
        r.download_mbps = -1.0;
        assert!(r.validate().is_err());
    }

    #[tokio::test]
    async fn insert_get_and_purge() {
        let dir = tempfile::tempdir().unwrap();
        let pool = connect(&dir.path().join("t.sqlite3")).await.unwrap();
        let r = NewResult {
            download_mbps: 940.5,
            upload_mbps: 410.0,
            ping_ms: 2.1,
            jitter_ms: 0.3,
        };
        let s = insert(&pool, &r, "100.80.1.2", Some("Orange"), Some("ua"), 90)
            .await
            .unwrap();
        assert_eq!(get(&pool, &s.share_token).await.unwrap().unwrap(), s);
        assert!(s.expires_at.is_some());

        let forever = insert(&pool, &r, "100.80.1.3", None, None, 0).await.unwrap();
        assert!(forever.expires_at.is_none());

        // Force expiry of the first one.
        sqlx::query("UPDATE results SET expires_at = '2000-01-01T00:00:00.000Z' WHERE share_token = ?")
            .bind(&s.share_token)
            .execute(&pool)
            .await
            .unwrap();
        assert!(get(&pool, &s.share_token).await.unwrap().is_none());
        assert_eq!(purge_expired(&pool).await.unwrap(), 1);
        assert!(get(&pool, &forever.share_token).await.unwrap().is_some());
        assert_eq!(recent(&pool, 10).await.unwrap().len(), 1);
    }
}
