use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::{
    error::AppError,
    middleware::auth::AuthUser,
    routes::auth_routes::ApiResponse,
    services::portfolio_service::{
        self, CashBalanceResponse, PortfolioSummary, SerializedHolding, SerializedPortfolio,
    },
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct CreatePortfolioRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePortfolioRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

async fn list_portfolios(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<ApiResponse<Vec<SerializedPortfolio>>>, AppError> {
    let portfolios = portfolio_service::list_user_portfolios(&state.pool, &auth.user_id).await?;
    Ok(Json(ApiResponse {
        success: true,
        data: portfolios,
    }))
}

async fn create_portfolio(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<CreatePortfolioRequest>,
) -> Result<Json<ApiResponse<SerializedPortfolio>>, AppError> {
    let portfolio = portfolio_service::create_portfolio(
        &state.pool,
        &auth.user_id,
        &payload.name,
        payload.description,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: portfolio,
    }))
}

async fn get_portfolio(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
) -> Result<Json<ApiResponse<SerializedPortfolio>>, AppError> {
    let portfolio =
        portfolio_service::get_portfolio_by_id(&state.pool, &auth.user_id, &portfolio_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: portfolio,
    }))
}

async fn update_portfolio(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
    Json(payload): Json<UpdatePortfolioRequest>,
) -> Result<Json<ApiResponse<SerializedPortfolio>>, AppError> {
    let portfolio = portfolio_service::update_portfolio(
        &state.pool,
        &auth.user_id,
        &portfolio_id,
        payload.name,
        payload.description,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: portfolio,
    }))
}

async fn delete_portfolio(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
) -> Result<Json<ApiResponse<SerializedPortfolio>>, AppError> {
    let portfolio =
        portfolio_service::deactivate_portfolio(&state.pool, &auth.user_id, &portfolio_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: portfolio,
    }))
}

async fn reactivate_portfolio(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
) -> Result<Json<ApiResponse<SerializedPortfolio>>, AppError> {
    let portfolio =
        portfolio_service::reactivate_portfolio(&state.pool, &auth.user_id, &portfolio_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: portfolio,
    }))
}

async fn get_portfolio_holdings(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<SerializedHolding>>>, AppError> {
    let holdings =
        portfolio_service::list_portfolio_holdings(&state.pool, &auth.user_id, &portfolio_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: holdings,
    }))
}

async fn get_cash_balance(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
) -> Result<Json<ApiResponse<CashBalanceResponse>>, AppError> {
    let balance =
        portfolio_service::get_portfolio_cash_balance(&state.pool, &auth.user_id, &portfolio_id)
            .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: balance,
    }))
}

async fn get_portfolio_summary(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(portfolio_id): Path<String>,
) -> Result<Json<ApiResponse<PortfolioSummary>>, AppError> {
    let summary =
        portfolio_service::get_portfolio_summary(&state.pool, &auth.user_id, &portfolio_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: summary,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_portfolios).post(create_portfolio))
        .route(
            "/{portfolioId}",
            get(get_portfolio)
                .patch(update_portfolio)
                .delete(delete_portfolio),
        )
        .route("/{portfolioId}/reactivate", post(reactivate_portfolio))
        .route("/{portfolioId}/holdings", get(get_portfolio_holdings))
        .route("/{portfolioId}/cash-balance", get(get_cash_balance))
        .route("/{portfolioId}/summary", get(get_portfolio_summary))
}
