use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

use crate::error::AppError;
use crate::models::Notification;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationMeta {
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    #[serde(rename = "totalPages")]
    pub total_pages: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationListResponse {
    pub notifications: Vec<Notification>,
    pub pagination: PaginationMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkAllReadResponse {
    pub updated: i64,
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

pub async fn list_notifications(
    pool: &PgPool,
    user_id: &str,
    page: Option<i64>,
    limit: Option<i64>,
    is_read: Option<bool>,
) -> Result<NotificationListResponse, AppError> {
    ensure_active_user(pool, user_id).await?;

    let page_num = page.unwrap_or(1).max(1);
    let limit_num = limit.unwrap_or(20).clamp(1, 100);
    let offset = (page_num - 1) * limit_num;

    let total: i64 = if let Some(read) = is_read {
        let r = sqlx::query(
            r#"SELECT COUNT(*) as count FROM "Notification" WHERE "userId" = $1 AND "isRead" = $2"#
        )
        .bind(user_id)
        .bind(read)
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to count notifications: {}", e)))?;
        r.get("count")
    } else {
        let r = sqlx::query(
            r#"SELECT COUNT(*) as count FROM "Notification" WHERE "userId" = $1"#
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to count notifications: {}", e)))?;
        r.get("count")
    };

    let total_pages = if total == 0 {
        0
    } else {
        (total + limit_num - 1) / limit_num
    };

    let notifications = if let Some(read) = is_read {
        sqlx::query_as::<_, Notification>(
            r#"
            SELECT *
            FROM "Notification"
            WHERE "userId" = $1 AND "isRead" = $2
            ORDER BY "createdAt" DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(user_id)
        .bind(read)
        .bind(limit_num)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to fetch notifications: {}", e)))?
    } else {
        sqlx::query_as::<_, Notification>(
            r#"
            SELECT *
            FROM "Notification"
            WHERE "userId" = $1
            ORDER BY "createdAt" DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit_num)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to fetch notifications: {}", e)))?
    };

    Ok(NotificationListResponse {
        notifications,
        pagination: PaginationMeta {
            page: page_num,
            limit: limit_num,
            total,
            total_pages,
        },
    })
}

pub async fn mark_notification_read(
    pool: &PgPool,
    user_id: &str,
    notification_id: &str,
) -> Result<Notification, AppError> {
    ensure_active_user(pool, user_id).await?;

    let notification = sqlx::query_as::<_, Notification>(
        r#"SELECT * FROM "Notification" WHERE id = $1 AND "userId" = $2"#
    )
    .bind(notification_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to query notification: {}", e)))?;

    let notif = match notification {
        Some(n) => n,
        None => return Err(AppError::not_found("Notification not found.", "NOTIFICATION_NOT_FOUND")),
    };

    if notif.is_read {
        return Ok(notif);
    }

    let now = Utc::now();
    let updated = sqlx::query_as::<_, Notification>(
        r#"
        UPDATE "Notification"
        SET "isRead" = true, "readAt" = $1
        WHERE id = $2
        RETURNING *
        "#,
    )
    .bind(now)
    .bind(notification_id)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to mark notification read: {}", e)))?;

    Ok(updated)
}

pub async fn mark_all_notifications_read(
    pool: &PgPool,
    user_id: &str,
) -> Result<MarkAllReadResponse, AppError> {
    ensure_active_user(pool, user_id).await?;

    let now = Utc::now();
    let result = sqlx::query(
        r#"
        UPDATE "Notification"
        SET "isRead" = true, "readAt" = $1
        WHERE "userId" = $2 AND "isRead" = false
        "#,
    )
    .bind(now)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to mark all notifications read: {}", e)))?;

    Ok(MarkAllReadResponse {
        updated: result.rows_affected() as i64,
    })
}
