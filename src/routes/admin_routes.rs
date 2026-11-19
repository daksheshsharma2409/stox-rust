use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};

use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::services::admin_service::{
    self, AdminOrderFilter, AdminPortfolioFilter, AdminUserFilter,
};
use crate::state::AppState;

fn require_admin(auth: &AuthUser) -> Result<(), AppError> {
    if !auth.role.eq_ignore_ascii_case("ADMIN") {
        return Err(AppError::forbidden("Admin access required.", "FORBIDDEN"));
    }
    Ok(())
}

async fn list_admin_orders_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Query(filter): Query<AdminOrderFilter>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&auth)?;
    let result = admin_service::list_admin_orders(&state.pool, filter).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": result
    })))
}

async fn list_admin_portfolios_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Query(filter): Query<AdminPortfolioFilter>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&auth)?;
    let result = admin_service::list_admin_portfolios(&state.pool, filter).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": result
    })))
}

async fn get_admin_portfolio_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(portfolio_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&auth)?;
    let portfolio = admin_service::get_admin_portfolio_by_id(&state.pool, &portfolio_id).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "portfolio": portfolio
        }
    })))
}

async fn list_admin_users_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Query(filter): Query<AdminUserFilter>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&auth)?;
    let result = admin_service::list_admin_users(&state.pool, filter).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": result
    })))
}

async fn get_admin_user_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&auth)?;
    let user = admin_service::get_admin_user_by_id(&state.pool, &user_id).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "user": user
        }
    })))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/orders", get(list_admin_orders_handler))
        .route("/portfolios", get(list_admin_portfolios_handler))
        .route("/portfolios/{portfolioId}", get(get_admin_portfolio_handler))
        .route("/users", get(list_admin_users_handler))
        .route("/users/{userId}", get(get_admin_user_handler))
}
