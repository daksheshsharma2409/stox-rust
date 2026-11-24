use std::str::FromStr;
use std::time::Duration;
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::Row;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::models::AlertCondition;
use crate::services::market_data_service::{MarketDataService, QuoteOptions};
use crate::services::trade_service;
use crate::state::{AppState, RealtimeMessage};

pub fn start_background_workers(state: AppState) {
    let order_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            if let Err(e) = trade_service::expire_overdue_orders(&order_state.pool, Utc::now()).await {
                error!("Order expiration worker error: {}", e);
            }
        }
    });

    let alert_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;
            if let Err(e) = run_alert_evaluation_cycle(&alert_state).await {
                debug!("Alert evaluation cycle: {}", e);
            }
        }
    });

    let league_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            if let Err(e) = run_league_status_sync(&league_state).await {
                debug!("League sync cycle: {}", e);
            }
        }
    });

    info!("All background workers initialized and running");
}

async fn run_alert_evaluation_cycle(state: &AppState) -> Result<(), String> {
    let alerts = sqlx::query(
        r#"
        SELECT id, "userId", symbol, condition, "targetPrice", "baselinePrice"
        FROM "Alert"
        WHERE status = 'ACTIVE'::"AlertStatus"
        LIMIT 50
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    let now = Utc::now();

    for r in alerts {
        let id: String = r.get("id");
        let user_id: String = r.get("userId");
        let symbol: String = r.get("symbol");
        let condition: AlertCondition = r.get("condition");
        let target_price: Decimal = r.get("targetPrice");
        let baseline_price: Option<Decimal> = r.get("baselinePrice");

        let quote_res = MarketDataService::get_quote(state, &symbol, &QuoteOptions::default()).await;
        let current_price = match quote_res {
            Ok(q) => match Decimal::from_str(&q.price) {
                Ok(p) => p,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        let triggered = match condition {
            AlertCondition::PriceAbove => current_price >= target_price,
            AlertCondition::PriceBelow => current_price <= target_price,
            AlertCondition::PercentChangeUp => {
                if let Some(base) = baseline_price {
                    if base > Decimal::ZERO {
                        let pct = ((current_price - base) / base) * Decimal::from(100);
                        pct >= target_price
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            AlertCondition::PercentChangeDown => {
                if let Some(base) = baseline_price {
                    if base > Decimal::ZERO {
                        let pct = ((base - current_price) / base) * Decimal::from(100);
                        pct >= target_price
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        };

        if triggered {
            let update_res = sqlx::query(
                r#"
                UPDATE "Alert"
                SET status = 'TRIGGERED'::"AlertStatus",
                    "triggeredAt" = $1,
                    "triggeredPrice" = $2,
                    "updatedAt" = $1
                WHERE id = $3 AND status = 'ACTIVE'::"AlertStatus"
                "#,
            )
            .bind(now)
            .bind(current_price)
            .bind(&id)
            .execute(&state.pool)
            .await;

            if let Ok(res) = update_res {
                if res.rows_affected() > 0 {
                    // Create notification
                    let notif_id = Uuid::new_v4().to_string();
                    let message = format!(
                        "Alert triggered: {} has reached {}",
                        symbol, current_price
                    );

                    let _ = sqlx::query(
                        r#"
                        INSERT INTO "Notification" (
                            id, "userId", type, title, message, data,
                            "isRead", "deliveryStatus", "createdAt"
                        )
                        VALUES ($1, $2, 'PRICE_ALERT'::"NotificationType", 'Price Alert Triggered', $3, $4, false, 'DELIVERED'::"NotificationDeliveryStatus", $5)
                        "#,
                    )
                    .bind(notif_id)
                    .bind(&user_id)
                    .bind(&message)
                    .bind(serde_json::json!({
                        "alertId": id,
                        "symbol": symbol,
                        "triggeredPrice": current_price.to_string(),
                        "condition": format!("{:?}", condition)
                    }))
                    .bind(now)
                    .execute(&state.pool)
                    .await;

                    // Publish to real-time websocket channel
                    let _ = state.tx_realtime.send(RealtimeMessage {
                        channel: format!("notifications:{}", user_id),
                        event: "price_alert".to_string(),
                        payload: serde_json::json!({
                            "alertId": id,
                            "symbol": symbol,
                            "price": current_price.to_string()
                        }),
                    });
                }
            }
        }
    }

    Ok(())
}

async fn run_league_status_sync(state: &AppState) -> Result<(), String> {
    let now = Utc::now();

    // Transition UPCOMING -> ACTIVE
    let _ = sqlx::query(
        r#"
        UPDATE "League"
        SET status = 'ACTIVE'::"LeagueStatus", "updatedAt" = $1
        WHERE status = 'UPCOMING'::"LeagueStatus"
          AND "startDate" <= $1
          AND "endDate" > $1
        "#,
    )
    .bind(now)
    .execute(&state.pool)
    .await;

    // Transition ACTIVE -> COMPLETED
    let _ = sqlx::query(
        r#"
        UPDATE "League"
        SET status = 'COMPLETED'::"LeagueStatus", "updatedAt" = $1
        WHERE status = 'ACTIVE'::"LeagueStatus"
          AND "endDate" <= $1
        "#,
    )
    .bind(now)
    .execute(&state.pool)
    .await;

    Ok(())
}
