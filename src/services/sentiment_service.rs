use uuid::Uuid;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

use crate::error::AppError;
use crate::models::{Sentiment, SentimentVote};

pub const MIN_SAMPLE_THRESHOLD: i64 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentBreakdown {
    pub bullish: i64,
    pub bearish: i64,
    pub neutral: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentPercentages {
    pub bullish: f64,
    pub bearish: f64,
    pub neutral: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentResponse {
    pub symbol: String,
    pub window: String,
    #[serde(rename = "totalVotes")]
    pub total_votes: i64,
    #[serde(rename = "sampleThreshold")]
    pub sample_threshold: i64,
    #[serde(rename = "sampleThresholdMet")]
    pub sample_threshold_met: bool,
    pub classification: String,
    pub status: String,
    pub breakdown: SentimentBreakdown,
    pub percentages: SentimentPercentages,
    #[serde(rename = "userVote")]
    pub user_vote: Option<Sentiment>,
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

pub async fn record_vote(
    pool: &PgPool,
    user_id: &str,
    symbol: &str,
    sentiment: Sentiment,
) -> Result<SentimentVote, AppError> {
    ensure_active_user(pool, user_id).await?;

    let normalized_symbol = symbol.trim().to_uppercase();
    let now = Utc::now();
    let id = Uuid::new_v4().to_string();

    let vote = sqlx::query_as::<_, SentimentVote>(
        r#"
        INSERT INTO "SentimentVote" (id, "userId", symbol, sentiment, "createdAt", "updatedAt")
        VALUES ($1, $2, $3, $4, $5, $5)
        ON CONFLICT ("userId", symbol) DO UPDATE SET
            sentiment = EXCLUDED.sentiment,
            "updatedAt" = EXCLUDED."updatedAt"
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(&normalized_symbol)
    .bind(sentiment)
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to record sentiment vote: {}", e)))?;

    Ok(vote)
}

pub async fn get_sentiment(
    pool: &PgPool,
    symbol: &str,
    window: Option<&str>,
    viewer_id: Option<&str>,
    now: DateTime<Utc>,
) -> Result<SentimentResponse, AppError> {
    let normalized_symbol = symbol.trim().to_uppercase();
    let normalized_window = window.unwrap_or("all").to_lowercase();

    let window_start = match normalized_window.as_str() {
        "24h" => Some(now - Duration::hours(24)),
        "7d" => Some(now - Duration::days(7)),
        "30d" => Some(now - Duration::days(30)),
        _ => None,
    };

    let rows = if let Some(ws) = window_start {
        sqlx::query(
            r#"
            SELECT sv.sentiment, COUNT(*) as count
            FROM "SentimentVote" sv
            JOIN "User" u ON u.id = sv."userId"
            WHERE sv.symbol = $1
              AND u."isSuspended" = false
              AND sv."updatedAt" >= $2
            GROUP BY sv.sentiment
            "#,
        )
        .bind(&normalized_symbol)
        .bind(ws)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to query sentiment votes: {}", e)))?
    } else {
        sqlx::query(
            r#"
            SELECT sv.sentiment, COUNT(*) as count
            FROM "SentimentVote" sv
            JOIN "User" u ON u.id = sv."userId"
            WHERE sv.symbol = $1
              AND u."isSuspended" = false
            GROUP BY sv.sentiment
            "#,
        )
        .bind(&normalized_symbol)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to query sentiment votes: {}", e)))?
    };

    let mut bullish: i64 = 0;
    let mut bearish: i64 = 0;
    let mut neutral: i64 = 0;

    for r in rows {
        let sent: Sentiment = r.get("sentiment");
        let count: i64 = r.get("count");
        match sent {
            Sentiment::Bullish => bullish = count,
            Sentiment::Bearish => bearish = count,
            Sentiment::Neutral => neutral = count,
        }
    }

    let user_vote = if let Some(v_id) = viewer_id {
        let uv_row = sqlx::query(
            r#"SELECT sentiment FROM "SentimentVote" WHERE "userId" = $1 AND symbol = $2"#
        )
        .bind(v_id)
        .bind(&normalized_symbol)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to fetch viewer vote: {}", e)))?;

        uv_row.map(|r| r.get::<Sentiment, _>("sentiment"))
    } else {
        None
    };

    let total = bullish + bearish + neutral;
    let sample_threshold_met = total >= MIN_SAMPLE_THRESHOLD;

    let (classification, status, percentages) = if total == 0 {
        (
            "NO_DATA".to_string(),
            "ZERO_VOTES".to_string(),
            SentimentPercentages {
                bullish: 0.0,
                bearish: 0.0,
                neutral: 0.0,
            },
        )
    } else if total < MIN_SAMPLE_THRESHOLD {
        let pb = ((bullish as f64 / total as f64) * 10000.0).round() / 100.0;
        let pr = ((bearish as f64 / total as f64) * 10000.0).round() / 100.0;
        let pn = ((neutral as f64 / total as f64) * 10000.0).round() / 100.0;
        (
            "NO_DATA".to_string(),
            "INSUFFICIENT_SAMPLE".to_string(),
            SentimentPercentages {
                bullish: pb,
                bearish: pr,
                neutral: pn,
            },
        )
    } else {
        let pb = (bullish as f64 / total as f64) * 100.0;
        let pr = (bearish as f64 / total as f64) * 100.0;
        let pn = (neutral as f64 / total as f64) * 100.0;

        let pct = SentimentPercentages {
            bullish: (pb * 100.0).round() / 100.0,
            bearish: (pr * 100.0).round() / 100.0,
            neutral: (pn * 100.0).round() / 100.0,
        };

        if pb > 50.0 && bullish > bearish {
            ("BULLISH".to_string(), "BULLISH_CONSENSUS".to_string(), pct)
        } else if pr > 50.0 && bearish > bullish {
            ("BEARISH".to_string(), "BEARISH_CONSENSUS".to_string(), pct)
        } else {
            ("NEUTRAL".to_string(), "NEUTRAL_CONSENSUS".to_string(), pct)
        }
    };

    Ok(SentimentResponse {
        symbol: normalized_symbol,
        window: normalized_window,
        total_votes: total,
        sample_threshold: MIN_SAMPLE_THRESHOLD,
        sample_threshold_met,
        classification,
        status,
        breakdown: SentimentBreakdown {
            bullish,
            bearish,
            neutral,
        },
        percentages,
        user_vote,
    })
}
