use axum::{
    extract::{Extension, Path, State},
    response::{IntoResponse, Redirect, Response},
    Form,
};
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
            match field.as_ref() {
                "domain" => errors.domain = msg,
                "folder_path" => errors.folder_path = msg,
                _ => {}
            }
        }
    }
    if !errors.domain.is_empty() || !errors.folder_path.is_empty() {
        return Ok(AddSitePage::new(
            true,
            form.domain,
            form.folder_path.clone(),
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
            form.folder_path,
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

    let result = sqlx::query(
        "INSERT INTO sites (domain, folder_path, wordpress_installed, user_id) VALUES ($1, $2, $3, $4)",
    )
    .bind(&form.domain)
    .bind(&form.folder_path)
    .bind(install_wp)
    .bind(user_id.value())
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            // TODO: create site folder, add Caddy block, reload FrankenPHP, optionally install WordPress
            Ok(Redirect::to("/?created=1").into_response())
        }
        Err(e) => {
            let msg = if let sqlx::Error::Database(db) = &e {
                if db.is_unique_violation() {
                    "Domain or folder path already in use."
                } else {
                    "Database error. Please try again."
                }
            } else {
                "Failed to create site."
            };
            Ok(AddSitePage::new(
                true,
                form.domain,
                form.folder_path,
                install_wp,
                AddSiteErrors::default(),
                msg.to_string(),
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
