use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::AppError,
    models::enums::{AlertCondition, AlertStatus, AssetType},
    models::{Alert, SymbolModel, User},
    services::market_data_service::{MarketDataService, QuoteOptions},
    state::AppState,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializedAlert {
    pub id: String,
    pub user_id: String,
    pub symbol: Option<String>,
    pub asset_type: Option<String>,
    pub condition: AlertCondition,
    pub target_price: String,
    pub baseline_price: Option<String>,
    pub baseline_at: Option<DateTime<Utc>>,
    pub status: AlertStatus,
    pub send_email: bool,
    pub send_push: bool,
    pub triggered_at: Option<DateTime<Utc>>,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertPagination {
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    #[serde(rename = "totalPages")]
    pub total_pages: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListAlertsResponse {
    pub alerts: Vec<SerializedAlert>,
    pub pagination: AlertPagination,
}

#[derive(Debug, Deserialize)]
pub struct CreateAlertPayload {
    pub symbol: String,
    #[serde(rename = "assetType")]
    pub asset_type: Option<AssetType>,
    pub condition: AlertCondition,
    #[serde(rename = "targetPrice")]
    pub target_price: Decimal,
    #[serde(rename = "sendEmail", default)]
    pub send_email: bool,
    #[serde(rename = "sendPush", default)]
    pub send_push: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAlertPayload {
    pub condition: Option<AlertCondition>,
    #[serde(rename = "targetPrice")]
    pub target_price: Option<Decimal>,
    pub status: Option<AlertStatus>,
    #[serde(rename = "sendEmail")]
    pub send_email: Option<bool>,
    #[serde(rename = "sendPush")]
    pub send_push: Option<bool>,
}

pub struct AlertService;

impl AlertService {
    pub fn is_percentage_condition(condition: &AlertCondition) -> bool {
        matches!(
            condition,
            AlertCondition::PercentChangeUp | AlertCondition::PercentChangeDown
        )
    }

    async fn ensure_active_user(pool: &PgPool, user_id: &str) -> Result<User, AppError> {
        let user = sqlx::query_as::<_, User>(r#"SELECT * FROM "User" WHERE "id" = $1"#)
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::not_found("User not found.", "USER_NOT_FOUND"))?;

        if user.is_suspended {
            return Err(AppError::forbidden(
                "Account is suspended.",
                "ACCOUNT_SUSPENDED",
            ));
        }

        Ok(user)
    }

    pub async fn list_alerts(
        pool: &PgPool,
        user_id: &str,
        page: i64,
        limit: i64,
        status: Option<AlertStatus>,
        symbol: Option<String>,
    ) -> Result<ListAlertsResponse, AppError> {
        Self::ensure_active_user(pool, user_id).await?;

        let offset = (page - 1) * limit;

        let alerts = sqlx::query_as::<_, Alert>(
            r#"
            SELECT a.* FROM "Alert" a
            LEFT JOIN "Symbol" s ON a."symbolId" = s."id"
            WHERE a."userId" = $1
              AND ($2::"AlertStatus" IS NULL OR a."status" = $2)
              AND ($3::text IS NULL OR s."symbol" = $3)
            ORDER BY a."createdAt" DESC
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(user_id)
        .bind(status)
        .bind(symbol.as_deref())
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let total_row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM "Alert" a
            LEFT JOIN "Symbol" s ON a."symbolId" = s."id"
            WHERE a."userId" = $1
              AND ($2::"AlertStatus" IS NULL OR a."status" = $2)
              AND ($3::text IS NULL OR s."symbol" = $3)
            "#,
        )
        .bind(user_id)
        .bind(status)
        .bind(symbol.as_deref())
        .fetch_one(pool)
        .await?;

        let total = total_row.0;
        let mut serialized = Vec::new();

        for alert in alerts {
            let symbol_row = sqlx::query_as::<_, SymbolModel>(
                r#"SELECT * FROM "Symbol" WHERE "id" = $1"#,
            )
            .bind(&alert.symbol_id)
            .fetch_optional(pool)
            .await?;

            serialized.push(SerializedAlert {
                id: alert.id,
                user_id: alert.user_id,
                symbol: symbol_row.as_ref().map(|s| s.symbol.clone()),
                asset_type: symbol_row.as_ref().map(|s| format!("{:?}", s.asset_type).to_uppercase()),
                condition: alert.condition,
                target_price: alert.target_price.to_string(),
                baseline_price: alert.baseline_price.map(|p| p.to_string()),
                baseline_at: alert.baseline_at,
                status: alert.status,
                send_email: alert.send_email,
                send_push: alert.send_push,
                triggered_at: alert.triggered_at,
                message: alert.message,
                created_at: alert.created_at,
                updated_at: alert.updated_at,
            });
        }

        let total_pages = if limit > 0 {
            (total + limit - 1) / limit
        } else {
            1
        };

        Ok(ListAlertsResponse {
            alerts: serialized,
            pagination: AlertPagination {
                page,
                limit,
                total,
                total_pages,
            },
        })
    }

    pub async fn create_alert(
        state: &AppState,
        user_id: &str,
        payload: CreateAlertPayload,
    ) -> Result<SerializedAlert, AppError> {
        Self::ensure_active_user(&state.pool, user_id).await?;

        // Find or create Symbol in DB
        let upper_symbol = payload.symbol.trim().to_uppercase();
        let symbol_record = sqlx::query_as::<_, SymbolModel>(
            r#"SELECT * FROM "Symbol" WHERE "symbol" = $1"#,
        )
        .bind(&upper_symbol)
        .fetch_optional(&state.pool)
        .await?;

        let symbol_model = match symbol_record {
            Some(s) => {
                if !s.is_active {
                    return Err(AppError::not_found("Active symbol not found.", "SYMBOL_NOT_FOUND"));
                }
                if let Some(ref req_asset) = payload.asset_type {
                    if &s.asset_type != req_asset {
                        return Err(AppError::bad_request(
                            "Asset type does not match the selected symbol.",
                            "ASSET_TYPE_MISMATCH",
                        ));
                    }
                }
                s
            }
            None => {
                // If symbol doesn't exist, auto-create it
                let inferred_asset = payload.asset_type.unwrap_or_else(|| {
                    if upper_symbol.contains("USDT") || upper_symbol.contains('/') || upper_symbol.starts_with("BINANCE:") {
                        AssetType::Crypto
                    } else {
                        AssetType::Stock
                    }
                });
                let new_id = Uuid::new_v4().to_string();
                sqlx::query_as::<_, SymbolModel>(
                    r#"
                    INSERT INTO "Symbol" ("id", "symbol", "name", "assetType", "isActive", "createdAt", "updatedAt")
                    VALUES ($1, $2, $3, $4, true, NOW(), NOW())
                    RETURNING *
                    "#,
                )
                .bind(&new_id)
                .bind(&upper_symbol)
                .bind(&upper_symbol)
                .bind(inferred_asset)
                .fetch_one(&state.pool)
                .await?
            }
        };

        let is_pct = Self::is_percentage_condition(&payload.condition);
        let (baseline_price, baseline_at) = if is_pct {
            let quote_opts = QuoteOptions {
                asset_class: Some(format!("{:?}", symbol_model.asset_type).to_uppercase()),
                reject_stale: false,
                max_age_seconds: None,
                bypass_cache: false,
            };
            let quote = MarketDataService::get_quote(state, &symbol_model.symbol, &quote_opts).await?;
            let price_dec = quote.price.parse::<Decimal>().unwrap_or(Decimal::ZERO);
            (Some(price_dec), Some(Utc::now()))
        } else {
            (None, None)
        };

        let alert_id = Uuid::new_v4().to_string();
        let alert = sqlx::query_as::<_, Alert>(
            r#"
            INSERT INTO "Alert" (
                "id", "userId", "symbolId", "condition", "targetPrice", "status",
                "sendEmail", "sendPush", "baselinePrice", "baselineAt", "createdAt", "updatedAt"
            )
            VALUES ($1, $2, $3, $4, $5, 'ACTIVE', $6, $7, $8, $9, NOW(), NOW())
            RETURNING *
            "#,
        )
        .bind(&alert_id)
        .bind(user_id)
        .bind(&symbol_model.id)
        .bind(payload.condition)
        .bind(payload.target_price)
        .bind(payload.send_email)
        .bind(payload.send_push)
        .bind(baseline_price)
        .bind(baseline_at)
        .fetch_one(&state.pool)
        .await?;

        Ok(SerializedAlert {
            id: alert.id,
            user_id: alert.user_id,
            symbol: Some(symbol_model.symbol),
            asset_type: Some(format!("{:?}", symbol_model.asset_type).to_uppercase()),
            condition: alert.condition,
            target_price: alert.target_price.to_string(),
            baseline_price: alert.baseline_price.map(|p| p.to_string()),
            baseline_at: alert.baseline_at,
            status: alert.status,
            send_email: alert.send_email,
            send_push: alert.send_push,
            triggered_at: alert.triggered_at,
            message: alert.message,
            created_at: alert.created_at,
            updated_at: alert.updated_at,
        })
    }

    pub async fn update_alert(
        state: &AppState,
        user_id: &str,
        alert_id: &str,
        payload: UpdateAlertPayload,
    ) -> Result<SerializedAlert, AppError> {
        Self::ensure_active_user(&state.pool, user_id).await?;

        let existing = sqlx::query_as::<_, Alert>(
            r#"SELECT * FROM "Alert" WHERE "id" = $1 AND "userId" = $2"#,
        )
        .bind(alert_id)
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("Alert not found.", "ALERT_NOT_FOUND"))?;

        let symbol_model = sqlx::query_as::<_, SymbolModel>(
            r#"SELECT * FROM "Symbol" WHERE "id" = $1"#,
        )
        .bind(&existing.symbol_id)
        .fetch_one(&state.pool)
        .await?;

        let effective_condition = payload.condition.unwrap_or(existing.condition);
        let effective_status = payload.status.unwrap_or(existing.status);
        let effective_target = payload.target_price.unwrap_or(existing.target_price);
        let effective_email = payload.send_email.unwrap_or(existing.send_email);
        let effective_push = payload.send_push.unwrap_or(existing.send_push);

        let rearming = effective_status == AlertStatus::Active && existing.status != AlertStatus::Active;
        let is_pct = Self::is_percentage_condition(&effective_condition);

        let (baseline_price, baseline_at) = if is_pct {
            let needs_baseline = !Self::is_percentage_condition(&existing.condition)
                || rearming
                || existing.baseline_price.is_none();

            if needs_baseline {
                let quote_opts = QuoteOptions {
                    asset_class: Some(format!("{:?}", symbol_model.asset_type).to_uppercase()),
                    reject_stale: false,
                    max_age_seconds: None,
                    bypass_cache: false,
                };
                let quote = MarketDataService::get_quote(state, &symbol_model.symbol, &quote_opts).await?;
                let price_dec = quote.price.parse::<Decimal>().unwrap_or(Decimal::ZERO);
                (Some(price_dec), Some(Utc::now()))
            } else {
                (existing.baseline_price, existing.baseline_at)
            }
        } else {
            (None, None)
        };

        let triggered_at = if effective_status == AlertStatus::Triggered && existing.status != AlertStatus::Triggered {
            Some(Utc::now())
        } else {
            existing.triggered_at
        };

        let updated = sqlx::query_as::<_, Alert>(
            r#"
            UPDATE "Alert"
            SET "condition" = $1,
                "targetPrice" = $2,
                "status" = $3,
                "sendEmail" = $4,
                "sendPush" = $5,
                "baselinePrice" = $6,
                "baselineAt" = $7,
                "triggeredAt" = $8,
                "updatedAt" = NOW()
            WHERE "id" = $9 AND "userId" = $10
            RETURNING *
            "#,
        )
        .bind(effective_condition)
        .bind(effective_target)
        .bind(effective_status)
        .bind(effective_email)
        .bind(effective_push)
        .bind(baseline_price)
        .bind(baseline_at)
        .bind(triggered_at)
        .bind(alert_id)
        .bind(user_id)
        .fetch_one(&state.pool)
        .await?;

        Ok(SerializedAlert {
            id: updated.id,
            user_id: updated.user_id,
            symbol: Some(symbol_model.symbol),
            asset_type: Some(format!("{:?}", symbol_model.asset_type).to_uppercase()),
            condition: updated.condition,
            target_price: updated.target_price.to_string(),
            baseline_price: updated.baseline_price.map(|p| p.to_string()),
            baseline_at: updated.baseline_at,
            status: updated.status,
            send_email: updated.send_email,
            send_push: updated.send_push,
            triggered_at: updated.triggered_at,
            message: updated.message,
            created_at: updated.created_at,
            updated_at: updated.updated_at,
        })
    }

    pub async fn delete_alert(
        pool: &PgPool,
        user_id: &str,
        alert_id: &str,
    ) -> Result<serde_json::Value, AppError> {
        Self::ensure_active_user(pool, user_id).await?;

        let res = sqlx::query(
            r#"DELETE FROM "Alert" WHERE "id" = $1 AND "userId" = $2"#,
        )
        .bind(alert_id)
        .bind(user_id)
        .execute(pool)
        .await?;

        if res.rows_affected() == 0 {
            return Err(AppError::not_found("Alert not found.", "ALERT_NOT_FOUND"));
        }

        Ok(serde_json::json!({ "id": alert_id }))
    }
}
