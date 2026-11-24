use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::AppError,
    middleware::auth::AuthUser,
    services::auth_service,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub name: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    #[serde(rename = "refreshToken")]
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    #[serde(rename = "refreshToken")]
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct GoogleLoginRequest {
    pub credential: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub name: Option<String>,
    #[serde(rename = "avatarUrl")]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

fn extract_client_info(headers: &HeaderMap) -> (Option<String>, Option<String>) {
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let ip_address = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string());

    (user_agent, ip_address)
}

async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<ApiResponse<auth_service::AuthResponseData>>, AppError> {
    let (user_agent, ip_address) = extract_client_info(&headers);
    let data = auth_service::register_user(
        &state.pool,
        &state.config,
        &payload.name,
        &payload.email,
        &payload.password,
        user_agent,
        ip_address,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<ApiResponse<auth_service::AuthResponseData>>, AppError> {
    let (user_agent, ip_address) = extract_client_info(&headers);
    let data = auth_service::login_user(
        &state.pool,
        &state.config,
        &payload.email,
        &payload.password,
        user_agent,
        ip_address,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RefreshRequest>,
) -> Result<Json<ApiResponse<auth_service::AuthResponseData>>, AppError> {
    let (user_agent, ip_address) = extract_client_info(&headers);
    let data = auth_service::refresh_access_token(
        &state.pool,
        &state.config,
        &payload.refresh_token,
        user_agent,
        ip_address,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

async fn logout(
    State(state): State<AppState>,
    Json(payload): Json<LogoutRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    auth_service::revoke_refresh_token(&state.pool, &payload.refresh_token).await;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "message": "Logged out successfully." }),
    }))
}

async fn google(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<GoogleLoginRequest>,
) -> Result<Json<ApiResponse<auth_service::AuthResponseData>>, AppError> {
    let (user_agent, ip_address) = extract_client_info(&headers);
    let data = auth_service::login_with_google(
        &state.pool,
        &state.config,
        &state.http_client,
        &payload.credential,
        user_agent,
        ip_address,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

async fn current_user(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<ApiResponse<crate::models::SanitizedUser>>, AppError> {
    let user = auth_service::get_current_user(&state.pool, &auth.user_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: user,
    }))
}

async fn update_current_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<UpdateProfileRequest>,
) -> Result<Json<ApiResponse<crate::models::SanitizedUser>>, AppError> {
    let user = auth_service::update_profile(
        &state.pool,
        &auth.user_id,
        payload.name,
        payload.avatar_url,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: user,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
        .route("/google", post(google))
        .route("/me", get(current_user))
        .route("/profile", patch(update_current_user))
}
