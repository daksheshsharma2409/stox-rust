use uuid::Uuid;
use chrono::Utc;
use serde::Deserialize;
use sqlx::{PgPool, Row};

use crate::error::AppError;
use crate::models::UserPreference;

#[derive(Debug, Deserialize)]
pub struct UpdatePreferencesRequest {
    #[serde(rename = "emailNotifications")]
    pub email_notifications: Option<bool>,
    #[serde(rename = "pushNotifications")]
    pub push_notifications: Option<bool>,
}

async fn ensure_active_user(pool: &PgPool, user_id: &str) -> Result<(), AppError> {
    let row = sqlx::query(
        r#"SELECT "isSuspended" FROM "User" WHERE id = $1"#
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to query user: {}", e)))?;

    match row {
        Some(r) => {
            let is_suspended: bool = r.get("isSuspended");
            if is_suspended {
                return Err(AppError::forbidden("Account is suspended.", "ACCOUNT_SUSPENDED"));
            }
            Ok(())
        }
        None => Err(AppError::not_found("User not found.", "USER_NOT_FOUND")),
    }
}

pub async fn get_preferences(
    pool: &PgPool,
    user_id: &str,
) -> Result<UserPreference, AppError> {
    ensure_active_user(pool, user_id).await?;

    let existing = sqlx::query_as::<_, UserPreference>(
        r#"SELECT * FROM "UserPreference" WHERE "userId" = $1"#
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to fetch preferences: {}", e)))?;

    if let Some(pref) = existing {
        return Ok(pref);
    }

    // Create lazily
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let pref = sqlx::query_as::<_, UserPreference>(
        r#"
        INSERT INTO "UserPreference" (id, "userId", "emailNotifications", "pushNotifications", "createdAt", "updatedAt")
        VALUES ($1, $2, true, true, $3, $3)
        ON CONFLICT ("userId") DO UPDATE SET "updatedAt" = EXCLUDED."updatedAt"
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to create preferences: {}", e)))?;

    Ok(pref)
}

pub async fn update_preferences(
    pool: &PgPool,
    user_id: &str,
    req: UpdatePreferencesRequest,
) -> Result<UserPreference, AppError> {
    ensure_active_user(pool, user_id).await?;

    let current = get_preferences(pool, user_id).await?;

    let new_email = req.email_notifications.unwrap_or(current.email_notifications);
    let new_push = req.push_notifications.unwrap_or(current.push_notifications);
    let now = Utc::now();

    let updated = sqlx::query_as::<_, UserPreference>(
        r#"
        UPDATE "UserPreference"
        SET "emailNotifications" = $1, "pushNotifications" = $2, "updatedAt" = $3
        WHERE "userId" = $4
        RETURNING *
        "#,
    )
    .bind(new_email)
    .bind(new_push)
    .bind(now)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to update preferences: {}", e)))?;

    Ok(updated)
}
