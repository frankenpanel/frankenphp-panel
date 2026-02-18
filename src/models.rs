use chrono::{DateTime, Utc};
use serde::Deserialize;
use validator::Validate;

#[derive(Debug, sqlx::FromRow)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct Site {
    pub id: i32,
    pub domain: String,
    pub folder_path: String,
    pub wordpress_installed: bool,
    pub user_id: i32,
    pub created_at: DateTime<Utc>,
    pub php_version: String,
}

#[derive(Debug, sqlx::FromRow)]
pub struct SiteWithStatus {
    pub id: i32,
    pub domain: String,
    pub folder_path: String,
    pub wordpress_installed: bool,
    pub user_id: i32,
    pub created_at: DateTime<Utc>,
    pub status: Option<String>,
    pub php_version: String,
}

#[derive(Debug, sqlx::FromRow)]
pub struct SiteDatabase {
    pub id: i32,
    pub site_id: i32,
    pub db_name: String,
    pub db_user: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LoginForm {
    #[validate(length(min = 1, message = "Username is required"))]
    pub username: String,
    #[validate(length(min = 1, message = "Password is required"))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AddSiteForm {
    #[validate(length(min = 1, message = "Domain is required"))]
    pub domain: String,
    pub install_wordpress: Option<String>,
    /// PHP version (e.g. 8.1, 8.2, 8.3)
    pub php_version: Option<String>,
    /// WordPress site title (required when install_wordpress=1)
    pub wp_title: Option<String>,
    /// WordPress admin username (required when install_wordpress=1)
    pub wp_admin_user: Option<String>,
    /// WordPress admin password (required when install_wordpress=1)
    pub wp_admin_password: Option<String>,
    /// WordPress admin email (required when install_wordpress=1)
    pub wp_admin_email: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AddDatabaseForm {
    #[validate(length(min = 1, max = 64, message = "Database name: 1–64 characters"))]
    pub database_name: String,
    #[validate(length(min = 1, max = 32, message = "Username: 1–32 characters"))]
    pub username: String,
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
}

pub fn validate_domain(domain: &str) -> Result<(), String> {
    let re = regex::Regex::new(
        r"^([a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}$|^localhost$",
    )
    .unwrap();
    if re.is_match(domain) {
        Ok(())
    } else {
        Err("Invalid domain format. Use a valid hostname (e.g. example.com).".to_string())
    }
}

pub fn validate_db_identifier(s: &str, name: &str) -> Result<(), String> {
    let re = regex::Regex::new(r"^[a-zA-Z0-9_]+$").unwrap();
    if re.is_match(s) {
        Ok(())
    } else {
        Err(format!(
            "{} may only contain letters, numbers, and underscores.",
            name
        ))
    }
}
