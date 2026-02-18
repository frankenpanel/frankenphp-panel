use axum::extract::{Extension, State};

use crate::auth::UserId;
use crate::state::AppState;
use crate::templates::{DashboardPage, DashboardSiteRow};

pub async fn dashboard(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
) -> impl axum::response::IntoResponse {
    let sites: Vec<DashboardSiteRow> = sqlx::query_as::<_, crate::models::SiteWithStatus>(
        "SELECT id, domain, folder_path, wordpress_installed, user_id, created_at, NULL::text AS status, php_version FROM sites WHERE user_id = $1 ORDER BY domain",
    )
    .bind(user_id.value())
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|s| DashboardSiteRow {
        id: s.id,
        domain: s.domain,
        folder_path: s.folder_path,
        wordpress_installed: s.wordpress_installed,
        user_id: s.user_id,
        created_at: s.created_at,
        status: s.status.unwrap_or_else(|| "unknown".to_string()),
        php_version: s.php_version,
    })
    .collect();

    let username = sqlx::query_scalar::<_, String>("SELECT username FROM users WHERE id = $1")
        .bind(user_id.value())
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten();

    DashboardPage {
        logged_in: username.is_some(),
        sites,
    }
}
