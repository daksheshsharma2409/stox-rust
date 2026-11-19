pub mod alert_routes;
pub mod auth_routes;
pub mod copy_trading_routes;
pub mod market_routes;
pub mod order_routes;
pub mod portfolio_routes;
pub mod social_routes;
pub mod trade_routes;
pub mod user_routes;

use axum::Router;
use crate::state::AppState;

pub fn api_v1_router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth_routes::routes())
        .nest("/users", user_routes::routes())
        .nest("/portfolios", portfolio_routes::routes())
        .nest("/orders", order_routes::routes())
        .nest("/trades", trade_routes::routes())
        .nest("/market", market_routes::routes())
        .nest("/alerts", alert_routes::routes())
        .nest("/social", social_routes::routes())
        .nest("/copy-trading", copy_trading_routes::routes())
}
