use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::Result;
use crate::state::AppState;

pub const SESSION_COOKIE: &str = "panel_session";
const SESSION_MAX_AGE_DAYS: i64 = 7;

pub async fn create_session(pool: &DbPool, user_id: i32) -> Result<String> {
    let token = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO sessions (token, user_id, expires_at) VALUES ($1, $2, NOW() + ($3 || ' days')::interval)",
    )
    .bind(&token)
    .bind(user_id)
    .bind(SESSION_MAX_AGE_DAYS)
    .execute(pool)
    .await?;
    Ok(token)
}

pub async fn get_user_id_from_session(pool: &DbPool, token: &str) -> Result<Option<i32>> {
    let row: Option<(i32,)> = sqlx::query_as(
        "SELECT user_id FROM sessions WHERE token = $1 AND expires_at > NOW()",
    )
    .bind(token)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

pub async fn delete_session(pool: &DbPool, token: &str) -> Result<()> {
    sqlx::query("DELETE FROM sessions WHERE token = $1")
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn require_auth(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response> {
    let cookie = request
        .headers()
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|c| {
            c.split("; ")
                .find(|p| p.starts_with(SESSION_COOKIE))
                .and_then(|p| p.strip_prefix(&format!("{}=", SESSION_COOKIE)))
                .map(|s| s.trim_matches('"').to_string())
        });
    let token = match cookie {
        Some(t) if !t.is_empty() => t,
        _ => return Ok(Redirect::to("/login").into_response()),
    };
    let user_id = match get_user_id_from_session(&state.pool, &token).await {
        Ok(Some(id)) => id,
        _ => return Ok(Redirect::to("/login").into_response()),
    };
    let mut request = request;
    request.extensions_mut().insert(UserId(user_id));
    Ok(next.run(request).await)
}

#[derive(Clone, Copy)]
pub struct UserId(pub i32);

impl UserId {
    pub fn value(self) -> i32 {
        self.0
    }
}
