use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};

use crate::{
    error::AppError,
    middleware::auth::{AuthUser, FromRef},
    state::AppState,
};

#[derive(Debug, Clone)]
pub struct OptionalAuthUser(pub Option<AuthUser>);

impl<S> FromRequestParts<S> for OptionalAuthUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        if parts.headers.get(axum::http::header::AUTHORIZATION).is_none() {
            return Ok(OptionalAuthUser(None));
        }

        let auth_user = AuthUser::from_request_parts(parts, state).await?;
        Ok(OptionalAuthUser(Some(auth_user)))
    }
}
