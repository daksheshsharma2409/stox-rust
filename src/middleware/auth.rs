use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

use crate::{error::AppError, state::AppState};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    #[serde(rename = "userId")]
    pub user_id: String,
    pub role: String,
    pub email: String,
    pub iss: Option<String>,
    pub sub: Option<String>,
    pub exp: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: String,
    pub role: String,
    pub email: String,
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);

        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .ok_or_else(|| {
                AppError::unauthorized("Authentication required.", "UNAUTHORIZED")
            })?;

        let auth_str = auth_header.to_str().map_err(|_| {
            AppError::unauthorized("Invalid authorization header.", "INVALID_AUTH_HEADER")
        })?;

        let mut parts_iter = auth_str.splitn(2, ' ');
        let scheme = parts_iter.next().unwrap_or("");
        let token = parts_iter.next().unwrap_or("");

        if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
            return Err(AppError::unauthorized(
                "Invalid authorization header.",
                "INVALID_AUTH_HEADER",
            ));
        }

        let mut validation = Validation::default();
        validation.set_issuer(&["stox"]);
        validation.validate_exp = true;

        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(app_state.config.jwt_access_secret.as_bytes()),
            &validation,
        )
        .map_err(|_| {
            AppError::unauthorized(
                "Invalid or expired access token.",
                "INVALID_ACCESS_TOKEN",
            )
        })?;

        let claims = token_data.claims;
        if claims.user_id.is_empty() {
            return Err(AppError::unauthorized(
                "Invalid access token.",
                "INVALID_ACCESS_TOKEN",
            ));
        }

        Ok(AuthUser {
            user_id: claims.user_id,
            role: claims.role,
            email: claims.email,
        })
    }
}

// Helper trait to allow extract state from S
pub trait FromRef<T> {
    fn from_ref(input: &T) -> Self;
}

impl FromRef<AppState> for AppState {
    fn from_ref(input: &AppState) -> Self {
        input.clone()
    }
}
