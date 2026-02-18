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
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let pool = db::create_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;

    let state = AppState { pool };

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
