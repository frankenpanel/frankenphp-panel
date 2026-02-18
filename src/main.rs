use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use frankenphp_panel::{config::Config, db, handlers, state::AppState};
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 {
        match args[1].as_str() {
            "set-admin-password" => {
                let password = args
                    .get(2)
                    .ok_or_else(|| anyhow::anyhow!("Usage: frankenphp-panel set-admin-password <password>"))?;
                return run_set_admin_password(password).await;
            }
            "migrate" => return run_migrate_only().await,
            _ => {}
        }
    }

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let pool = db::create_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;

    let state = AppState {
        pool,
        config: config.clone(),
    };

    let public = Router::new()
        .route("/login", get(handlers::get_login).post(handlers::post_login))
        .route("/logout", post(handlers::logout))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state.clone());

    let private = Router::new()
        .route("/", get(handlers::dashboard))
        .route("/sites/new", get(handlers::new_site))
        .route("/sites", post(handlers::create_site))
        .route("/sites/:id", get(handlers::site_detail))
        .route("/sites/:id/restart", post(handlers::restart_site))
        .route("/sites/:id/delete", post(handlers::delete_site))
        .route("/databases/new", get(handlers::new_database))
        .route("/databases", post(handlers::create_database))
        .route("/databases/:id/delete", post(handlers::delete_database))
        .layer(middleware::from_fn_with_state(state.clone(), frankenphp_panel::auth::require_auth))
        .with_state(state.clone());

    let app = public.merge(private);

    let addr = config.bind;
    tracing::info!("Panel listening on http://{}", addr);
    axum::serve(
        tokio::net::TcpListener::bind(addr).await?,
        app,
    )
    .await?;
    Ok(())
}

async fn run_migrate_only() -> anyhow::Result<()> {
    let config = Config::from_env();
    let pool = db::create_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;
    println!("Migrations completed.");
    Ok(())
}

async fn run_set_admin_password(password: &str) -> anyhow::Result<()> {
    let config = Config::from_env();
    let pool = db::create_pool(&config.database_url).await?;
    let hash = bcrypt::hash(password, 12).map_err(|e| anyhow::anyhow!("bcrypt: {}", e))?;
    let rows = sqlx::query("UPDATE users SET password_hash = $1 WHERE username = 'admin'")
        .bind(&hash)
        .execute(&pool)
        .await?;
    if rows.rows_affected() == 0 {
        anyhow::bail!("No user 'admin' found. Run migrations first.");
    }
    println!("Admin password updated.");
    Ok(())
}
