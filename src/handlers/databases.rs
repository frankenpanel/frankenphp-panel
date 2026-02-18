use axum::{
    extract::{Extension, Query, State},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use serde::Deserialize;

use crate::auth::UserId;
use crate::error::Result;
use crate::models::validate_db_identifier;
use crate::state::AppState;
use crate::templates::{AddDatabaseErrors, AddDatabasePage};

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
        "SELECT id, domain, folder_path, wordpress_installed, user_id, created_at FROM sites WHERE user_id = $1 ORDER BY domain",
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
}

pub async fn create_database(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Form(form): Form<CreateDatabaseForm>,
) -> Result<Response> {
    let mut errors = AddDatabaseErrors::default();
    let site_id: i32 = form.site_id.parse().unwrap_or(0);

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
        let sites = sqlx::query_as::<_, crate::models::Site>(
            "SELECT id, domain, folder_path, wordpress_installed, user_id, created_at FROM sites WHERE user_id = $1 ORDER BY domain",
        )
        .bind(user_id.value())
        .fetch_all(&state.pool)
        .await?;
        return Ok(AddDatabasePage::new(
            true,
            sites,
            site_id,
            form.database_name,
            form.username,
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
        let sites = sqlx::query_as::<_, crate::models::Site>(
            "SELECT id, domain, folder_path, wordpress_installed, user_id, created_at FROM sites WHERE user_id = $1 ORDER BY domain",
        )
        .bind(user_id.value())
        .fetch_all(&state.pool)
        .await?;
        return Ok(AddDatabasePage::new(
            true,
            sites,
            site_id,
            form.database_name,
            form.username,
            AddDatabaseErrors::default(),
            "Site not found.".to_string(),
        )
        .into_response());
    }

    let result = sqlx::query(
        "INSERT INTO site_databases (site_id, db_name, db_user) VALUES ($1, $2, $3)",
    )
    .bind(site_id)
    .bind(&form.database_name)
    .bind(&form.username)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            // TODO: create MariaDB/MySQL database and user via safe command
            Ok(Redirect::to("/?db_created=1").into_response())
        }
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
            let sites = sqlx::query_as::<_, crate::models::Site>(
                "SELECT id, domain, folder_path, wordpress_installed, user_id, created_at FROM sites WHERE user_id = $1 ORDER BY domain",
            )
            .bind(user_id.value())
            .fetch_all(&state.pool)
            .await?;
            Ok(AddDatabasePage::new(
                true,
                sites,
                site_id,
                form.database_name,
                form.username,
                AddDatabaseErrors::default(),
                msg.to_string(),
            )
            .into_response())
        }
    }
}
