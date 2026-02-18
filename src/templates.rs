use askama::Template;

use crate::models::{Site, SiteDatabase};

/// View type for dashboard table rows (status as String for template display).
pub struct DashboardSiteRow {
    pub id: i32,
    pub domain: String,
    pub folder_path: String,
    pub wordpress_installed: bool,
    pub user_id: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub status: String,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginPage {
    pub username: String,
    pub errors: LoginErrors,
    pub error_message: String,
}

#[derive(Default)]
pub struct LoginErrors {
    pub username: String,
    pub password: String,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardPage {
    pub logged_in: bool,
    pub sites: Vec<DashboardSiteRow>,
}

#[derive(Template)]
#[template(path = "add_site.html")]
pub struct AddSitePage {
    pub logged_in: bool,
    pub domain: String,
    pub install_wordpress: bool,
    pub errors: AddSiteErrors,
    pub error_message: String,
}

#[derive(Default)]
pub struct AddSiteErrors {
    pub domain: String,
    pub folder_path: String,
}

#[derive(Template)]
#[template(path = "add_database.html")]
pub struct AddDatabasePage {
    pub logged_in: bool,
    pub sites: Vec<Site>,
    pub site_id: i32,
    pub database_name: String,
    pub username: String,
    pub errors: AddDatabaseErrors,
    pub error_message: String,
}

#[derive(Default)]
pub struct AddDatabaseErrors {
    pub site_id: String,
    pub database_name: String,
    pub username: String,
    pub password: String,
}

#[derive(Template)]
#[template(path = "site_detail.html")]
pub struct SiteDetailPage {
    pub logged_in: bool,
    pub site: Site,
    pub databases: Vec<SiteDatabase>,
    pub ssl_status: String,
}

impl LoginPage {
    pub fn new(username: String, errors: LoginErrors, error_message: String) -> Self {
        Self {
            username,
            errors,
            error_message,
        }
    }
    pub fn with_error(mut self, message: String) -> Self {
        self.error_message = message;
        self
    }
}

impl AddSitePage {
    pub fn new(
        logged_in: bool,
        domain: String,
        install_wordpress: bool,
        errors: AddSiteErrors,
        error_message: String,
    ) -> Self {
        Self {
            logged_in,
            domain,
            install_wordpress,
            errors,
            error_message,
        }
    }
}

impl AddDatabasePage {
    pub fn new(
        logged_in: bool,
        sites: Vec<Site>,
        site_id: i32,
        database_name: String,
        username: String,
        errors: AddDatabaseErrors,
        error_message: String,
    ) -> Self {
        Self {
            logged_in,
            sites,
            site_id,
            database_name,
            username,
            errors,
            error_message,
        }
    }
}