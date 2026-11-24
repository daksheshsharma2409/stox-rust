pub mod achievement_routes;
pub mod admin_routes;
pub mod alert_routes;
pub mod auth_routes;
pub mod copy_trading_routes;
pub mod health_routes;
pub mod leaderboard_routes;
pub mod league_routes;
pub mod market_routes;
pub mod notification_routes;
pub mod order_routes;
pub mod portfolio_routes;
pub mod preference_routes;
pub mod sentiment_routes;
pub mod social_routes;
pub mod trade_routes;
pub mod user_routes;

use axum::Router;
use crate::state::AppState;

pub fn api_v1_router() -> Router<AppState> {
    Router::new()
        .nest("/health", health_routes::routes())
        .nest("/auth", auth_routes::routes())
        .nest("/users", user_routes::routes())
        .nest("/portfolios", portfolio_routes::routes())
        .nest("/orders", order_routes::routes())
        .nest("/trades", trade_routes::routes())
        .nest("/market", market_routes::routes())
        .nest("/alerts", alert_routes::routes())
        .nest("/social", social_routes::routes())
        .nest("/copy-trading", copy_trading_routes::routes())
        .nest("/leaderboard", leaderboard_routes::routes())
        .nest("/leagues", league_routes::routes())
        .nest("/achievements", achievement_routes::routes())
        .nest("/sentiment", sentiment_routes::routes())
        .nest("/preferences", preference_routes::routes())
        .nest("/notifications", notification_routes::routes())
        .nest("/admin", admin_routes::routes())
}
