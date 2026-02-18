use std::path::PathBuf;
use std::net::SocketAddr;

#[derive(Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub database_url: String,
    pub session_secret: String,
    /// If set, run this script (via sudo) when creating a site: script <domain> <folder_path>
    pub site_create_script: Option<PathBuf>,
}

impl Config {
    pub fn from_env() -> Self {
        let site_create_script = std::env::var("SITE_CREATE_SCRIPT")
            .ok()
            .map(PathBuf::from)
            .filter(|p| p.exists());
        Self {
            bind: std::env::var("PANEL_BIND")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| "127.0.0.1:2090".parse().unwrap()),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://panel:panel@127.0.0.1/panel".to_string()),
            session_secret: std::env::var("PANEL_SESSION_SECRET")
                .unwrap_or_else(|_| "change-me-in-production-min-32-chars!!".to_string()),
            site_create_script,
        }
    }
}
