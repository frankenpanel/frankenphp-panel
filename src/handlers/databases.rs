use axum::{
    extract::{Extension, Path, Query, State},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use serde::Deserialize;
use std::process::Stdio;
use tokio::process::Command;

use crate::auth::UserId;
use crate::error::{AppError, Result};
use crate::models::validate_db_identifier;
use crate::state::AppState;
use crate::templates::{AddDatabaseErrors, AddDatabasePage};

async fn fetch_user_sites(pool: &crate::db::DbPool, user_id: i32) -> Result<Vec<crate::models::Site>> {
    sqlx::query_as::<_, crate::models::Site>(
        "SELECT id, domain, folder_path, wordpress_installed, user_id, created_at, php_version FROM sites WHERE user_id = $1 ORDER BY domain",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

#[derive(Debug, Deserialize)]
pub struct NewDbQuery {
    pub site_id: Option<i32>,
}

pub async fn new_database(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Query(q): Query<NewDbQuery>,
) -> Result<impl IntoResponse> {
    let sites = sqlx::query_as::<_, crate::models::Site>(
        "SELECT id, domain, folder_path, wordpress_installed, user_id, created_at, php_version FROM sites WHERE user_id = $1 ORDER BY domain",
    )
    .bind(user_id.value())
    .fetch_all(&state.pool)
    .await?;

    Ok(AddDatabasePage::new(
        true,
        sites,
        q.site_id.unwrap_or(0),
        String::new(),
        String::new(),
        "full".to_string(),
        AddDatabaseErrors::default(),
        String::new(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct CreateDatabaseForm {
    pub site_id: String,
    #[serde(rename = "database_name")]
    pub database_name: String,
    pub username: String,
    pub password: String,
    pub privileges: Option<String>,
}

fn normalize_privileges(priv_str: Option<&String>) -> String {
    match priv_str.map(|s| s.trim()).unwrap_or("") {
        "readonly" => "readonly".to_string(),
        _ => "full".to_string(),
    }
}

pub async fn create_database(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Form(form): Form<CreateDatabaseForm>,
) -> Result<Response> {
    let mut errors = AddDatabaseErrors::default();
    let site_id: i32 = form.site_id.parse().unwrap_or(0);
    let privileges = normalize_privileges(form.privileges.as_ref());

    if site_id == 0 {
        errors.site_id = "Please select a site.".to_string();
    }
    if let Err(msg) = validate_db_identifier(&form.database_name, "Database name") {
        errors.database_name = msg;
    }
    if let Err(msg) = validate_db_identifier(&form.username, "Username") {
        errors.username = msg;
    }
    if form.database_name.len() > 64 {
        errors.database_name = "Database name must be 64 characters or less.".to_string();
    }
    if form.username.len() > 32 {
        errors.username = "Username must be 32 characters or less.".to_string();
    }
    if form.password.len() < 8 {
        errors.password = "Password must be at least 8 characters.".to_string();
    }

    if !errors.site_id.is_empty()
        || !errors.database_name.is_empty()
        || !errors.username.is_empty()
        || !errors.password.is_empty()
    {
        let sites = fetch_user_sites(&state.pool, user_id.value()).await?;
        return Ok(AddDatabasePage::new(
            true,
            sites,
            site_id,
            form.database_name,
            form.username,
            privileges.clone(),
            errors,
            String::new(),
        )
        .into_response());
    }

    let site_exists = sqlx::query_scalar::<_, i32>("SELECT id FROM sites WHERE id = $1 AND user_id = $2")
        .bind(site_id)
        .bind(user_id.value())
        .fetch_optional(&state.pool)
        .await?;
    if site_exists.is_none() {
        let sites = fetch_user_sites(&state.pool, user_id.value()).await?;
        return Ok(AddDatabasePage::new(
            true,
            sites,
            site_id,
            form.database_name,
            form.username,
            privileges.clone(),
            AddDatabaseErrors::default(),
            "Site not found.".to_string(),
        )
        .into_response());
    }

    if let Some(ref script) = state.config.db_create_script {
        let mut cmd = Command::new("sudo");
        cmd.arg(script.as_os_str())
            .arg(&form.database_name)
            .arg(&form.username)
            .arg(&form.password)
            .arg(&privileges)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = cmd.output().await;
        if let Ok(out) = output {
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let sites = fetch_user_sites(&state.pool, user_id.value()).await?;
                return Ok(AddDatabasePage::new(
                    true,
                    sites,
                    site_id,
                    form.database_name,
                    form.username,
                    privileges.clone(),
                    AddDatabaseErrors::default(),
                    format!("Could not create database on server: {} {}", stderr, stdout).chars().take(300).collect::<String>(),
                )
                .into_response());
            }
        } else if let Err(e) = output {
            let sites = fetch_user_sites(&state.pool, user_id.value()).await?;
            return Ok(AddDatabasePage::new(
                true,
                sites,
                site_id,
                form.database_name,
                form.username,
                privileges,
                AddDatabaseErrors::default(),
                format!("Could not run database script: {}", e),
            )
            .into_response());
        }
    }

    let result = sqlx::query(
        "INSERT INTO site_databases (site_id, db_name, db_user, privileges) VALUES ($1, $2, $3, $4)",
    )
    .bind(site_id)
    .bind(&form.database_name)
    .bind(&form.username)
    .bind(&privileges)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => Ok(Redirect::to("/?db_created=1").into_response()),
        Err(e) => {
            let msg = if let sqlx::Error::Database(db) = &e {
                if db.is_unique_violation() {
                    "This database name or user already exists for this site."
                } else {
                    "Database error. Please try again."
                }
            } else {
                "Failed to create database."
            };
            let sites = fetch_user_sites(&state.pool, user_id.value()).await?;
            Ok(AddDatabasePage::new(
                true,
                sites,
                site_id,
                form.database_name,
                form.username,
                privileges,
                AddDatabaseErrors::default(),
                msg.to_string(),
            )
            .into_response())
        }
    }
}

pub async fn delete_database(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Path(id): Path<i32>,
) -> Result<Response> {
    let row = sqlx::query_as::<_, (i32, String, String)>(
        "SELECT sd.id, sd.db_name, sd.db_user FROM site_databases sd JOIN sites s ON s.id = sd.site_id WHERE sd.id = $1 AND s.user_id = $2",
    )
    .bind(id)
    .bind(user_id.value())
    .fetch_optional(&state.pool)
    .await?;

    let (_, db_name, db_user) = row.ok_or(AppError::Validation("Database not found.".into()))?;

    if let Some(ref script) = state.config.db_delete_script {
        let mut cmd = Command::new("sudo");
        cmd.arg(script.as_os_str())
            .arg(&db_name)
            .arg(&db_user)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let _ = cmd.output().await;
    }

    sqlx::query("DELETE FROM site_databases WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;

    Ok(Redirect::to("/?db_deleted=1").into_response())
}
