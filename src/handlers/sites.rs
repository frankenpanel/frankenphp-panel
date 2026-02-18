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

pub async fn new_site(
    State(_state): State<AppState>,
    Extension(_user_id): Extension<UserId>,
) -> Result<impl IntoResponse> {
    Ok(AddSitePage::new(
        true,
        String::new(),
        false,
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
    if !errors.domain.is_empty() {
        return Ok(AddSitePage::new(
            true,
            form.domain,
            form.install_wordpress.as_deref() == Some("1"),
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
            AddSiteErrors {
                domain: msg,
                ..Default::default()
            },
            String::new(),
        )
        .into_response());
    }

    let install_wp = form.install_wordpress.as_deref() == Some("1");
    let folder_path = format!("/var/www/{}", form.domain.trim());

    // Create site directory and Caddy config, reload Caddy (if script is configured)
    if let Some(ref script) = state.config.site_create_script {
        let script_path = script.as_os_str();
        let domain = form.domain.trim().to_string();
        let output = Command::new("sudo")
            .arg(script_path)
            .arg(&domain)
            .arg(&folder_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        match output {
            Ok(out) if !out.status.success() => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let stdout = String::from_utf8_lossy(&out.stdout);
                tracing::warn!("site-create script failed: {} {}", stdout, stderr);
                return Ok(AddSitePage::new(
                    true,
                    form.domain,
                    install_wp,
                    AddSiteErrors {
                        folder_path: "Could not create site on server (folder/Caddy). Check logs or run the site-create script manually.".to_string(),
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
        "INSERT INTO sites (domain, folder_path, wordpress_installed, user_id) VALUES ($1, $2, $3, $4)",
    )
    .bind(&form.domain)
    .bind(&folder_path)
    .bind(install_wp)
    .bind(user_id.value())
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
        "SELECT id, domain, folder_path, wordpress_installed, user_id, created_at FROM sites WHERE id = $1 AND user_id = $2",
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

    Ok(SiteDetailPage {
        logged_in: true,
        site,
        databases,
        ssl_status: "active".to_string(), // TODO: real SSL check
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
