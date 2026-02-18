use std::net::SocketAddr;

#[derive(Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub database_url: String,
    pub session_secret: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            bind: std::env::var("PANEL_BIND")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| "127.0.0.1:2090".parse().unwrap()),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://panel:panel@127.0.0.1/panel".to_string()),
            session_secret: std::env::var("PANEL_SESSION_SECRET")
                .unwrap_or_else(|_| "change-me-in-production-min-32-chars!!".to_string()),
        }
    }
}
