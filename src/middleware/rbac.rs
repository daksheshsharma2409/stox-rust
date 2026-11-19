use crate::{error::AppError, middleware::auth::AuthUser};

pub fn require_admin(user: &AuthUser) -> Result<(), AppError> {
    if user.role != "ADMIN" {
        return Err(AppError::forbidden(
            "Administrator access required.",
            "FORBIDDEN",
        ));
    }
    Ok(())
}
