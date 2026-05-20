use std::{env, path::PathBuf};

/// Runtime configuration that controls server/network/session behavior.
#[derive(Clone, Debug)]
pub struct AppConfig {
    /// Bind address for the axum server.
    pub http_bind: String,
    /// PostgreSQL connection string.
    pub database_url: String,
    /// Absolute/relative root path for uploaded files.
    pub storage_root: PathBuf,
    /// Maximum session lifetime in days.
    pub session_max_days: i64,
    /// Maximum inactivity window in hours.
    pub session_idle_hours: i64,
    /// Development captcha secret used by the gate endpoint.
    pub captcha_dev_secret: String,
    /// Session cookie name.
    pub session_cookie_name: String,
    /// HMAC secret for signing session cookies.
    pub session_cookie_secret: String,
    /// Whether cookie should include `Secure` attribute.
    pub session_cookie_secure: bool,
}

impl AppConfig {
    /// Loads configuration from environment and falls back to spec-safe defaults.
    pub fn from_env() -> anyhow::Result<Self> {
        // Bind server on all interfaces so reverse proxies (Caddy) can reach it.
        let http_bind = env::var("APP_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

        // Use local PostgreSQL by default for single-node deployment.
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:postgres@127.0.0.1:5432/novastrum_chat".to_string()
        });

        // Store files under local disk because the spec forbids external object storage.
        let storage_root = env::var("STORAGE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("storage"));

        // Enforce spec: max lifetime defaults to 7 days.
        let session_max_days = parse_i64_with_default("SESSION_MAX_DAYS", 7)?;

        // Enforce spec: inactivity timeout defaults to 24 hours.
        let session_idle_hours = parse_i64_with_default("SESSION_IDLE_HOURS", 24)?;

        // A placeholder captcha token for local development and integration tests.
        let captcha_dev_secret =
            env::var("CAPTCHA_DEV_SECRET").unwrap_or_else(|_| "i-am-human".to_string());

        // Session cookie settings.
        let session_cookie_name =
            env::var("SESSION_COOKIE_NAME").unwrap_or_else(|_| "novastrum_session".to_string());
        let session_cookie_secret = env::var("SESSION_COOKIE_SECRET")
            .unwrap_or_else(|_| "change-me-in-production".to_string());
        let session_cookie_secure = parse_bool_with_default("SESSION_COOKIE_SECURE", false)?;

        Ok(Self {
            http_bind,
            database_url,
            storage_root,
            session_max_days,
            session_idle_hours,
            captcha_dev_secret,
            session_cookie_name,
            session_cookie_secret,
            session_cookie_secure,
        })
    }
}

/// Parses an i64 environment value and falls back to the provided default.
fn parse_i64_with_default(key: &str, default: i64) -> anyhow::Result<i64> {
    // Missing key is acceptable because we intentionally support local defaults.
    let raw = match env::var(key) {
        Ok(value) => value,
        Err(_) => return Ok(default),
    };

    // Invalid numeric configuration is returned as a startup-time error.
    let parsed = raw
        .parse::<i64>()
        .map_err(|err| anyhow::anyhow!("{key} must be an integer: {err}"))?;

    Ok(parsed)
}

/// Parses a boolean environment value and falls back to the provided default.
fn parse_bool_with_default(key: &str, default: bool) -> anyhow::Result<bool> {
    let raw = match env::var(key) {
        Ok(value) => value,
        Err(_) => return Ok(default),
    };

    let parsed = raw
        .parse::<bool>()
        .map_err(|err| anyhow::anyhow!("{key} must be true/false: {err}"))?;

    Ok(parsed)
}
