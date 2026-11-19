pub mod auth_routes;
pub mod portfolio_routes;
pub mod user_routes;

use axum::Router;
use crate::state::AppState;

pub fn api_v1_router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth_routes::routes())
        .nest("/users", user_routes::routes())
        .nest("/portfolios", portfolio_routes::routes())
}
