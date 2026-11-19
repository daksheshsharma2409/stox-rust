use uuid::Uuid;
use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

use crate::error::AppError;
use crate::models::{Achievement, UserAchievement};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAchievementWithDetails {
    pub id: String,
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "achievementId")]
    pub achievement_id: String,
    pub reason: String,
    #[serde(rename = "unlockedAt")]
    pub unlocked_at: chrono::DateTime<Utc>,
    pub achievement: Achievement,
}

pub struct CatalogItem {
    pub code: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub category: &'static str,
    pub criteria: serde_json::Value,
}

pub fn get_catalog() -> Vec<CatalogItem> {
    vec![
        CatalogItem {
            code: "FIRST_TRADE",
            name: "First Trade",
            description: "Executed your first trade.",
            category: "TRADING",
            criteria: serde_json::json!({ "totalTrades": 1 }),
        },
        CatalogItem {
            code: "FIRST_PROFIT",
            name: "First Profit",
            description: "Closed your first profitable trade.",
            category: "TRADING",
            criteria: serde_json::json!({ "profitableTrades": 1 }),
        },
        CatalogItem {
            code: "TEN_SUCCESSFUL_TRADES",
            name: "Ten Profitable Trades",
            description: "Closed ten profitable trades.",
            category: "TRADING",
            criteria: serde_json::json!({ "profitableTrades": 10 }),
        },
        CatalogItem {
            code: "PORTFOLIO_MILESTONE",
            name: "Portfolio Milestone",
            description: "Doubled your portfolio starting balance.",
            category: "PORTFOLIO",
            criteria: serde_json::json!({ "multiplier": 2 }),
        },
    ]
}

pub async fn ensure_catalog(pool: &PgPool) -> Result<(), AppError> {
    let now = Utc::now();
    for item in get_catalog() {
        let id = Uuid::new_v4().to_string();
        let _ = sqlx::query(
            r#"
            INSERT INTO "Achievement" (id, code, name, description, category, criteria, "createdAt")
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (code) DO UPDATE SET
                name = EXCLUDED.name,
                description = EXCLUDED.description,
                category = EXCLUDED.category,
                criteria = EXCLUDED.criteria
            "#,
        )
        .bind(&id)
        .bind(item.code)
        .bind(item.name)
        .bind(item.description)
        .bind(item.category)
        .bind(&item.criteria)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to ensure achievement catalog: {}", e)))?;
    }
    Ok(())
}

pub async fn list_achievements(pool: &PgPool) -> Result<Vec<Achievement>, AppError> {
    ensure_catalog(pool).await?;

    let achievements = sqlx::query_as::<_, Achievement>(
        r#"
        SELECT *
        FROM "Achievement"
        ORDER BY "createdAt" ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to list achievements: {}", e)))?;

    Ok(achievements)
}

pub async fn list_user_achievements(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<UserAchievementWithDetails>, AppError> {
    ensure_catalog(pool).await?;

    let rows = sqlx::query(
        r#"
        SELECT
            ua.id,
            ua."userId",
            ua."achievementId",
            ua.reason,
            ua."unlockedAt",
            a.id as a_id,
            a.code as a_code,
            a.name as a_name,
            a.description as a_description,
            a."iconUrl" as a_icon_url,
            a.category as a_category,
            a.criteria as a_criteria,
            a."createdAt" as a_created_at
        FROM "UserAchievement" ua
        JOIN "Achievement" a ON a.id = ua."achievementId"
        WHERE ua."userId" = $1
        ORDER BY ua."unlockedAt" DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to list user achievements: {}", e)))?;

    let results = rows
        .into_iter()
        .map(|r| {
            let id: String = r.get("id");
            let u_id: String = r.get("userId");
            let achievement_id: String = r.get("achievementId");
            let reason: String = r.get("reason");
            let unlocked_at: chrono::DateTime<Utc> = r.get("unlockedAt");

            let a = Achievement {
                id: r.get("a_id"),
                code: r.get("a_code"),
                name: r.get("a_name"),
                description: r.get("a_description"),
                icon_url: r.get("a_icon_url"),
                category: r.get("a_category"),
                criteria: r.get("a_criteria"),
                created_at: r.get("a_created_at"),
            };

            UserAchievementWithDetails {
                id,
                user_id: u_id,
                achievement_id,
                reason,
                unlocked_at,
                achievement: a,
            }
        })
        .collect();

    Ok(results)
}

pub async fn grant_achievement(
    pool: &PgPool,
    user_id: &str,
    achievement_code: &str,
) -> Result<Option<UserAchievement>, AppError> {
    ensure_catalog(pool).await?;

    let achievement = sqlx::query_as::<_, Achievement>(
        r#"SELECT * FROM "Achievement" WHERE code = $1"#
    )
    .bind(achievement_code)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to find achievement: {}", e)))?;

    let achievement = match achievement {
        Some(a) => a,
        None => return Ok(None),
    };

    let user_achievement_id = Uuid::new_v4().to_string();
    let now = Utc::now();

    let inserted = sqlx::query_as::<_, UserAchievement>(
        r#"
        INSERT INTO "UserAchievement" (id, "userId", "achievementId", reason, "unlockedAt")
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT ("userId", "achievementId") DO NOTHING
        RETURNING *
        "#,
    )
    .bind(&user_achievement_id)
    .bind(user_id)
    .bind(&achievement.id)
    .bind(&achievement.description)
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to insert user achievement: {}", e)))?;

    if let Some(ref ua) = inserted {
        // Create notification
        let notif_id = Uuid::new_v4().to_string();
        let _ = sqlx::query(
            r#"
            INSERT INTO "Notification" (
                id, "userId", type, title, message, data,
                "isRead", "deliveryStatus", "createdAt"
            )
            VALUES ($1, $2, 'ACHIEVEMENT_UNLOCKED'::"NotificationType", $3, $4, $5, false, 'DELIVERED'::"NotificationDeliveryStatus", $6)
            "#,
        )
        .bind(notif_id)
        .bind(user_id)
        .bind("Achievement unlocked")
        .bind(format!("You earned the \"{}\" achievement!", achievement.name))
        .bind(serde_json::json!({
            "achievementId": achievement.id,
            "code": achievement.code,
            "name": achievement.name,
            "description": ua.reason,
            "reason": ua.reason
        }))
        .bind(now)
        .execute(pool)
        .await;
    }

    Ok(inserted)
}

pub async fn evaluate_and_grant_achievements(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<UserAchievement>, AppError> {
    ensure_catalog(pool).await?;

    let unlocked_codes: Vec<String> = sqlx::query(
        r#"
        SELECT a.code
        FROM "UserAchievement" ua
        JOIN "Achievement" a ON a.id = ua."achievementId"
        WHERE ua."userId" = $1
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to query unlocked achievements: {}", e)))?
    .into_iter()
    .map(|r| r.get("code"))
    .collect();

    let mut granted = Vec::new();

    // 1. FIRST_TRADE
    if !unlocked_codes.contains(&"FIRST_TRADE".to_string()) {
        let total_trades_row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM "Trade" t
            JOIN "Portfolio" p ON p.id = t."portfolioId"
            WHERE p."userId" = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to count trades: {}", e)))?;

        let total_trades: i64 = total_trades_row.get("count");
        if total_trades >= 1 {
            if let Some(ua) = grant_achievement(pool, user_id, "FIRST_TRADE").await? {
                granted.push(ua);
            }
        }
    }

    // 2. FIRST_PROFIT & TEN_SUCCESSFUL_TRADES
    if !unlocked_codes.contains(&"FIRST_PROFIT".to_string())
        || !unlocked_codes.contains(&"TEN_SUCCESSFUL_TRADES".to_string())
    {
        let profitable_trades_row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM "Trade" t
            JOIN "Portfolio" p ON p.id = t."portfolioId"
            WHERE p."userId" = $1 AND t."realizedPnl" > 0
            "#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to count profitable trades: {}", e)))?;

        let profitable_trades: i64 = profitable_trades_row.get("count");
        if !unlocked_codes.contains(&"FIRST_PROFIT".to_string()) && profitable_trades >= 1 {
            if let Some(ua) = grant_achievement(pool, user_id, "FIRST_PROFIT").await? {
                granted.push(ua);
            }
        }
        if !unlocked_codes.contains(&"TEN_SUCCESSFUL_TRADES".to_string()) && profitable_trades >= 10 {
            if let Some(ua) = grant_achievement(pool, user_id, "TEN_SUCCESSFUL_TRADES").await? {
                granted.push(ua);
            }
        }
    }

    // 3. PORTFOLIO_MILESTONE
    if !unlocked_codes.contains(&"PORTFOLIO_MILESTONE".to_string()) {
        let portfolios = sqlx::query(
            r#"
            SELECT id, "startingBalance", "cashBalance"
            FROM "Portfolio"
            WHERE "userId" = $1 AND "isActive" = true
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to fetch portfolios: {}", e)))?;

        for p in portfolios {
            let starting_balance: Decimal = p.get("startingBalance");
            let cash_balance: Decimal = p.get("cashBalance");
            let p_id: String = p.get("id");

            if starting_balance > Decimal::ZERO {
                // Calculate holding invested basis + cash balance
                let invested_row = sqlx::query(
                    r#"SELECT COALESCE(SUM("totalInvested"), 0) as invested FROM "Holding" WHERE "portfolioId" = $1"#
                )
                .bind(&p_id)
                .fetch_one(pool)
                .await
                .map_err(|e| AppError::internal(format!("Failed to fetch invested: {}", e)))?;

                let total_invested: Decimal = invested_row.get("invested");
                let approx_val = cash_balance + total_invested;

                if approx_val >= starting_balance * Decimal::from(2) {
                    if let Some(ua) = grant_achievement(pool, user_id, "PORTFOLIO_MILESTONE").await? {
                        granted.push(ua);
                    }
                    break;
                }
            }
        }
    }

    Ok(granted)
}
