pub mod config;
pub mod error;
pub mod middleware;
pub mod models;
pub mod observability;
pub mod realtime;
pub mod routes;
pub mod services;
pub mod state;
pub mod utils;
pub mod workers;

use std::sync::Arc;
use axum::{routing::get, Router};
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    config::AppConfig,
    middleware::{
        request_context::request_context_middleware,
        security_headers::security_headers_middleware,
    },
    observability::metrics::{init_metrics, metrics_handler},
    routes::api_v1_router,
    state::AppState,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,stox_backend_rust=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Arc::new(AppConfig::from_env().expect("Failed to load configuration"));

    if config.metrics_enabled {
        init_metrics();
    }

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .connect(&config.database_url)
        .await
        .expect("Failed to connect to PostgreSQL database");

    tracing::info!("Connected to PostgreSQL database");

    let state = AppState::new(pool, config.clone());

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .expose_headers([axum::http::HeaderName::from_static("x-request-id")]);

    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .nest("/api/v1", api_v1_router())
        .layer(axum::middleware::from_fn(security_headers_middleware))
        .layer(axum::middleware::from_fn(request_context_middleware))
        .layer(cors)
        .with_state(state);

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Stox backend (Rust) running on port {}", config.port);

    axum::serve(listener, app).await?;

    Ok(())
}
