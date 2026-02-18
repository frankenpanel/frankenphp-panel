use axum::{
    extract::{Extension, Path, State},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use std::process::Stdio;
use tokio::process::Command;
use validator::Validate;

use crate::auth::UserId;
use crate::error::{AppError, Result};
use crate::models::{validate_domain, AddSiteForm};
use crate::state::AppState;
use crate::templates::{AddSiteErrors, AddSitePage, SiteDetailPage};

fn wp_form_values(form: &AddSiteForm) -> (String, String, String) {
    (
        form.wp_title.as_deref().unwrap_or("").trim().to_string(),
        form.wp_admin_user.as_deref().unwrap_or("").trim().to_string(),
        form.wp_admin_email.as_deref().unwrap_or("").trim().to_string(),
    )
}

fn validate_wp_fields(form: &AddSiteForm) -> AddSiteErrors {
    let mut e = AddSiteErrors::default();
    let title = form.wp_title.as_deref().unwrap_or("").trim();
    let user = form.wp_admin_user.as_deref().unwrap_or("").trim();
    let pass = form.wp_admin_password.as_deref().unwrap_or("");
    let email = form.wp_admin_email.as_deref().unwrap_or("").trim();
    if title.is_empty() {
        e.wp_title = "Site title is required.".to_string();
    }
    if user.is_empty() {
        e.wp_admin_user = "Admin username is required.".to_string();
    } else if user.len() > 60 || !user.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        e.wp_admin_user = "Username: letters, numbers, and underscores only (max 60).".to_string();
    }
    if pass.len() < 8 {
        e.wp_admin_password = "Password must be at least 8 characters.".to_string();
    }
    if email.is_empty() {
        e.wp_admin_email = "Admin email is required.".to_string();
    } else if !email.contains('@') || !email.split('@').nth(1).map_or(false, |after| after.contains('.')) {
        e.wp_admin_email = "Enter a valid email address.".to_string();
    }
    e
}

fn has_wp_errors(e: &AddSiteErrors) -> bool {
    !e.wp_title.is_empty() || !e.wp_admin_user.is_empty() || !e.wp_admin_password.is_empty() || !e.wp_admin_email.is_empty()
}

pub async fn new_site(
    State(_state): State<AppState>,
    Extension(_user_id): Extension<UserId>,
) -> Result<impl IntoResponse> {
    Ok(AddSitePage::new(
        true,
        String::new(),
        false,
        "8.2".to_string(),
        String::new(),
        String::new(),
        String::new(),
        AddSiteErrors::default(),
        String::new(),
    ))
}

pub async fn create_site(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Form(form): Form<AddSiteForm>,
) -> Result<Response> {
    let mut errors = AddSiteErrors::default();
    if let Err(e) = form.validate() {
        for (field, err) in e.field_errors() {
            let msg = err
                .first()
                .and_then(|m| m.message.as_ref())
                .map(|m| m.to_string())
                .unwrap_or_else(|| "Invalid".to_string());
            if field == "domain" {
                errors.domain = msg;
            }
        }
    }
    let (wp_title, wp_admin_user, wp_admin_email) = wp_form_values(&form);
    let php_version = form
        .php_version
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("8.2")
        .to_string();
    if !errors.domain.is_empty() {
        return Ok(AddSitePage::new(
            true,
            form.domain,
            form.install_wordpress.as_deref() == Some("1"),
            php_version.clone(),
            wp_title,
            wp_admin_user,
            wp_admin_email,
            errors,
            String::new(),
        )
        .into_response());
    }

    if let Err(msg) = validate_domain(&form.domain) {
        return Ok(AddSitePage::new(
            true,
            form.domain,
            form.install_wordpress.as_deref() == Some("1"),
            php_version.clone(),
            wp_title.clone(),
            wp_admin_user.clone(),
            wp_admin_email.clone(),
            AddSiteErrors {
                domain: msg,
                ..Default::default()
            },
            String::new(),
        )
        .into_response());
    }

    let install_wp = form.install_wordpress.as_deref() == Some("1");
    if install_wp {
        let wp_errors = validate_wp_fields(&form);
        if has_wp_errors(&wp_errors) {
            return Ok(AddSitePage::new(
                true,
                form.domain,
                true,
                php_version.clone(),
                wp_title,
                wp_admin_user,
                wp_admin_email,
                wp_errors,
                String::new(),
            )
            .into_response());
        }
    }

    let folder_path = format!("/var/www/{}", form.domain.trim());

    // Create site directory and Caddy config, reload Caddy (if script is configured)
    if let Some(ref script) = state.config.site_create_script {
        let script_path = script.as_os_str();
        let domain = form.domain.trim().to_string();
        let wp_arg = if install_wp { "1" } else { "0" };
        let mut cmd = Command::new("sudo");
        cmd.arg(script_path)
            .arg(&domain)
            .arg(&folder_path)
            .arg(wp_arg)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if install_wp {
            cmd.arg(form.wp_title.as_deref().unwrap_or(""))
                .arg(form.wp_admin_user.as_deref().unwrap_or(""))
                .arg(form.wp_admin_password.as_deref().unwrap_or(""))
                .arg(form.wp_admin_email.as_deref().unwrap_or(""));
        } else {
            cmd.arg("").arg("").arg("").arg("");
        }
        cmd.arg(&php_version);
        let output = cmd.output().await;

        match output {
            Ok(out) if !out.status.success() => {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                tracing::warn!("site-create script failed: {} {}", stdout, stderr);
                let msg = if stderr.is_empty() && stdout.is_empty() {
                    "Could not create site on server (folder/Caddy). Check server logs.".to_string()
                } else {
                    let detail: String = format!("{} {}", stderr, stdout).chars().take(350).collect();
                    format!("Site setup failed: {}", detail.trim())
                };
                return Ok(AddSitePage::new(
                    true,
                    form.domain,
                    install_wp,
                    php_version.clone(),
                    wp_title,
                    wp_admin_user,
                    wp_admin_email,
                    AddSiteErrors {
                        folder_path: msg,
                        ..Default::default()
                    },
                    String::new(),
                )
                .into_response());
            }
            Err(e) => {
                tracing::warn!("site-create script error: {}", e);
                return Ok(AddSitePage::new(
                    true,
                    form.domain,
                    install_wp,
                    php_version.clone(),
                    wp_title,
                    wp_admin_user,
                    wp_admin_email,
                    AddSiteErrors {
                        folder_path: format!("Could not run site setup: {}. Ensure SITE_CREATE_SCRIPT is correct and the panel user can run it with sudo.", e),
                        ..Default::default()
                    },
                    String::new(),
                )
                .into_response());
            }
            _ => {}
        }
    }

    let result = sqlx::query(
        "INSERT INTO sites (domain, folder_path, wordpress_installed, user_id, php_version) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&form.domain)
    .bind(&folder_path)
    .bind(install_wp)
    .bind(user_id.value())
    .bind(&php_version)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => Ok(Redirect::to("/?created=1").into_response()),
        Err(e) => {
            let (err_msg, folder_error) = if let sqlx::Error::Database(db) = &e {
                if db.is_unique_violation() {
                    (
                        String::new(),
                        "A site with this domain or path already exists.".to_string(),
                    )
                } else {
                    ("Database error. Please try again.".to_string(), String::new())
                }
            } else {
                ("Failed to create site.".to_string(), String::new())
            };
            Ok(AddSitePage::new(
                true,
                form.domain,
                install_wp,
                php_version,
                wp_title,
                wp_admin_user,
                wp_admin_email,
                AddSiteErrors {
                    folder_path: folder_error,
                    ..Default::default()
                },
                err_msg,
            )
            .into_response())
        }
    }
}

pub async fn site_detail(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Path(id): Path<i32>,
) -> Result<Response> {
    let site = sqlx::query_as::<_, crate::models::Site>(
        "SELECT id, domain, folder_path, wordpress_installed, user_id, created_at, php_version FROM sites WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id.value())
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::Validation("Site not found.".to_string()))?;

    let databases = sqlx::query_as::<_, crate::models::SiteDatabase>(
        "SELECT id, site_id, db_name, db_user, created_at FROM site_databases WHERE site_id = $1",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    let site_ip = state
        .config
        .server_ip
        .as_deref()
        .unwrap_or("—")
        .to_string();
    let site_user = state
        .config
        .web_user
        .as_deref()
        .unwrap_or("www-data")
        .to_string();

    Ok(SiteDetailPage {
        logged_in: true,
        site,
        databases,
        ssl_status: "active".to_string(), // TODO: real SSL check
        site_ip,
        site_user,
    }
    .into_response())
}

pub async fn restart_site(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Path(id): Path<i32>,
) -> Result<Response> {
    let _exists = sqlx::query_scalar::<_, i32>("SELECT 1 FROM sites WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id.value())
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::Validation("Site not found.".to_string()))?;
    // TODO: trigger FrankenPHP reload for this site
    let path = format!("/sites/{}?restarted=1", id);
    let res = axum::response::Response::builder()
        .status(axum::http::StatusCode::FOUND)
        .header(axum::http::header::LOCATION, path)
        .body(axum::body::Body::empty())
        .unwrap();
    Ok(res)
}

pub async fn delete_site(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Path(id): Path<i32>,
) -> Result<Response> {
    let site = sqlx::query_as::<_, crate::models::Site>(
        "SELECT id, domain, folder_path, wordpress_installed, user_id, created_at, php_version FROM sites WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id.value())
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::Validation("Site not found.".into()))?;

    let databases = sqlx::query_as::<_, crate::models::SiteDatabase>(
        "SELECT id, site_id, db_name, db_user, created_at FROM site_databases WHERE site_id = $1",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    if let Some(ref script) = state.config.site_delete_script {
        let mut cmd = Command::new("sudo");
        cmd.arg(script.as_os_str())
            .arg(&site.domain)
            .arg(&site.folder_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for db in &databases {
            cmd.arg(&db.db_name).arg(&db.db_user);
        }
        let output = cmd.output().await;
        if let Ok(out) = output {
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                tracing::warn!("site-delete script failed: {} {}", stdout, stderr);
            }
        } else if let Err(e) = output {
            tracing::warn!("site-delete script error: {}", e);
        }
    }

    sqlx::query("DELETE FROM site_databases WHERE site_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    let r = sqlx::query("DELETE FROM sites WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id.value())
        .execute(&state.pool)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::Validation("Site not found.".into()));
    }
    Ok(Redirect::to("/?deleted=1").into_response())
}
