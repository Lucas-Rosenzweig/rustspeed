CREATE TABLE IF NOT EXISTS results (
    id            TEXT PRIMARY KEY NOT NULL,
    share_token   TEXT NOT NULL UNIQUE,
    created_at    TEXT NOT NULL,
    client_ip     TEXT NOT NULL,
    client_isp    TEXT,
    download_mbps REAL NOT NULL,
    upload_mbps   REAL NOT NULL,
    ping_ms       REAL NOT NULL,
    jitter_ms     REAL NOT NULL,
    user_agent    TEXT,
    expires_at    TEXT
);

CREATE INDEX IF NOT EXISTS idx_results_created_at ON results (created_at);
CREATE INDEX IF NOT EXISTS idx_results_expires_at ON results (expires_at);
