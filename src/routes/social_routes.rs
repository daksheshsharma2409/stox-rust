use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::{
    error::AppError,
    middleware::auth::AuthUser,
    routes::auth_routes::ApiResponse,
    services::social_service::{
        CreatePostPayload, FeedResponse, FollowerItem, PostEngagement, SerializedPost,
        SocialService,
    },
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct FeedQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/feed", get(get_feed))
        .route("/posts", post(create_post))
        .route("/posts/{postId}", get(get_post).delete(delete_post))
        .route("/posts/{postId}/engagement", get(get_engagement))
        .route("/posts/{postId}/likes", post(like_post).delete(unlike_post))
        .route("/followers", get(get_followers))
        .route("/following", get(get_following))
        .route("/follow/{userId}", post(follow_user).delete(unfollow_user))
}

async fn get_feed(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<FeedQuery>,
) -> Result<Json<ApiResponse<FeedResponse>>, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let sort = query.sort.as_deref().unwrap_or("latest");

    let res = SocialService::get_feed(
        &state.pool,
        Some(&auth.user_id),
        page,
        limit,
        sort,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn create_post(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<CreatePostPayload>,
) -> Result<Json<ApiResponse<SerializedPost>>, AppError> {
    let res = SocialService::create_post(&state.pool, &auth.user_id, payload).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn get_post(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(post_id): Path<String>,
) -> Result<Json<ApiResponse<SerializedPost>>, AppError> {
    let res = SocialService::get_post_by_id(&state.pool, &post_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn delete_post(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(post_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let res = SocialService::delete_post(&state.pool, &auth.user_id, &post_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn get_engagement(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(post_id): Path<String>,
) -> Result<Json<ApiResponse<PostEngagement>>, AppError> {
    let res = SocialService::get_post_engagement(&state.pool, &auth.user_id, &post_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn like_post(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(post_id): Path<String>,
) -> Result<Json<ApiResponse<PostEngagement>>, AppError> {
    let res = SocialService::like_post(&state.pool, &auth.user_id, &post_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn unlike_post(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(post_id): Path<String>,
) -> Result<Json<ApiResponse<PostEngagement>>, AppError> {
    let res = SocialService::unlike_post(&state.pool, &auth.user_id, &post_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn get_followers(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<ApiResponse<Vec<FollowerItem>>>, AppError> {
    let res = SocialService::list_followers(&state.pool, &auth.user_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn get_following(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<ApiResponse<Vec<FollowerItem>>>, AppError> {
    let res = SocialService::list_following(&state.pool, &auth.user_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn follow_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(user_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let res = SocialService::follow_user(&state.pool, &auth.user_id, &user_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}

async fn unfollow_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(user_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let res = SocialService::unfollow_user(&state.pool, &auth.user_id, &user_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: res,
    }))
}
