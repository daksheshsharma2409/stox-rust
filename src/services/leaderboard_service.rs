use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

use crate::error::AppError;
use crate::models::{LeaderboardPeriod, LeaderboardScope};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardUserSummary {
    pub id: String,
    pub name: String,
    #[serde(rename = "avatarUrl")]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntryItem {
    pub rank: i32,
    #[serde(rename = "userId")]
    pub user_id: String,
    pub user: Option<LeaderboardUserSummary>,
    pub scope: LeaderboardScope,
    pub period: LeaderboardPeriod,
    #[serde(rename = "totalValue")]
    pub total_value: String,
    #[serde(rename = "returnPct")]
    pub return_pct: String,
    #[serde(rename = "snapshotDate")]
    pub snapshot_date: NaiveDate,
    #[serde(rename = "createdAt")]
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationMeta {
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    #[serde(rename = "totalPages")]
    pub total_pages: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardResponse {
    pub leaderboard: Vec<LeaderboardEntryItem>,
    pub pagination: PaginationMeta,
}

fn parse_period(period_str: Option<&str>) -> (LeaderboardPeriod, &'static str) {
    match period_str.map(|s| s.to_uppercase()).as_deref() {
        Some("DAILY") => (LeaderboardPeriod::Daily, "DAILY"),
        Some("WEEKLY") => (LeaderboardPeriod::Weekly, "WEEKLY"),
        Some("MONTHLY") => (LeaderboardPeriod::Monthly, "MONTHLY"),
        _ => (LeaderboardPeriod::AllTime, "ALL_TIME"),
    }
}

pub async fn resolve_snapshot_date(
    pool: &PgPool,
    scope: &str,
    period: &str,
) -> Result<Option<NaiveDate>, AppError> {
    let row = sqlx::query(
        r#"
        SELECT "snapshotDate"
        FROM "LeaderboardSnapshot"
        WHERE scope = $1::"LeaderboardScope" AND period = $2::"LeaderboardPeriod"
        ORDER BY "snapshotDate" DESC
        LIMIT 1
        "#,
    )
    .bind(scope)
    .bind(period)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to query snapshot date: {}", e)))?;

    Ok(row.map(|r| r.get::<NaiveDate, _>("snapshotDate")))
}

pub async fn get_global_leaderboard(
    pool: &PgPool,
    period_str: Option<&str>,
    page: Option<i64>,
    limit: Option<i64>,
) -> Result<LeaderboardResponse, AppError> {
    let (period_enum, period_sql) = parse_period(period_str);
    let page_num = page.unwrap_or(1).max(1);
    let limit_num = limit.unwrap_or(20).clamp(1, 100);
    let offset = (page_num - 1) * limit_num;

    let snapshot_date = resolve_snapshot_date(pool, "GLOBAL", period_sql).await?;
    let snapshot_date = match snapshot_date {
        Some(date) => date,
        None => {
            return Ok(LeaderboardResponse {
                leaderboard: Vec::new(),
                pagination: PaginationMeta {
                    page: page_num,
                    limit: limit_num,
                    total: 0,
                    total_pages: 0,
                },
            });
        }
    };

    let total_row = sqlx::query(
        r#"
        SELECT COUNT(*) as count
        FROM "LeaderboardEntry" e
        JOIN "User" u ON u.id = e."userId"
        WHERE e.scope = 'GLOBAL'::"LeaderboardScope"
          AND e.period = $1::"LeaderboardPeriod"
          AND e."snapshotDate" = $2
          AND u."isSuspended" = false
        "#,
    )
    .bind(period_sql)
    .bind(snapshot_date)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to count leaderboard entries: {}", e)))?;

    let total: i64 = total_row.get("count");
    let total_pages = if total == 0 {
        0
    } else {
        (total + limit_num - 1) / limit_num
    };

    let rows = sqlx::query(
        r#"
        SELECT
            e.id,
            e."userId",
            e.rank,
            e."totalValue",
            e."returnPct",
            e."snapshotDate",
            e."createdAt",
            u.id as user_id,
            u.name as user_name,
            u."avatarUrl" as user_avatar_url
        FROM "LeaderboardEntry" e
        JOIN "User" u ON u.id = e."userId"
        WHERE e.scope = 'GLOBAL'::"LeaderboardScope"
          AND e.period = $1::"LeaderboardPeriod"
          AND e."snapshotDate" = $2
          AND u."isSuspended" = false
        ORDER BY e.rank ASC, e."userId" ASC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(period_sql)
    .bind(snapshot_date)
    .bind(limit_num)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to fetch leaderboard entries: {}", e)))?;

    let items = rows
        .into_iter()
        .map(|r| {
            let user_id: String = r.get("user_id");
            let user_name: String = r.get("user_name");
            let user_avatar_url: Option<String> = r.get("user_avatar_url");
            let rank: i32 = r.get("rank");
            let total_value: Decimal = r.get("totalValue");
            let return_pct: Decimal = r.get("returnPct");
            let snap_date: NaiveDate = r.get("snapshotDate");
            let created_at: chrono::DateTime<Utc> = r.get("createdAt");

            LeaderboardEntryItem {
                rank,
                user_id: user_id.clone(),
                user: Some(LeaderboardUserSummary {
                    id: user_id,
                    name: user_name,
                    avatar_url: user_avatar_url,
                }),
                scope: LeaderboardScope::Global,
                period: period_enum,
                total_value: total_value.to_string(),
                return_pct: return_pct.to_string(),
                snapshot_date: snap_date,
                created_at,
            }
        })
        .collect();

    Ok(LeaderboardResponse {
        leaderboard: items,
        pagination: PaginationMeta {
            page: page_num,
            limit: limit_num,
            total,
            total_pages,
        },
    })
}

pub async fn get_friends_leaderboard(
    pool: &PgPool,
    user_id: &str,
    period_str: Option<&str>,
    page: Option<i64>,
    limit: Option<i64>,
) -> Result<LeaderboardResponse, AppError> {
    let (period_enum, period_sql) = parse_period(period_str);
    let page_num = page.unwrap_or(1).max(1);
    let limit_num = limit.unwrap_or(20).clamp(1, 100);

    let snapshot_date = resolve_snapshot_date(pool, "GLOBAL", period_sql).await?;
    let snapshot_date = match snapshot_date {
        Some(date) => date,
        None => {
            return Ok(LeaderboardResponse {
                leaderboard: Vec::new(),
                pagination: PaginationMeta {
                    page: page_num,
                    limit: limit_num,
                    total: 0,
                    total_pages: 0,
                },
            });
        }
    };

    // User + users that user_id follows
    let rows = sqlx::query(
        r#"
        SELECT
            e.id,
            e."userId",
            e."totalValue",
            e."returnPct",
            e."snapshotDate",
            e."createdAt",
            u.id as user_id,
            u.name as user_name,
            u."avatarUrl" as user_avatar_url
        FROM "LeaderboardEntry" e
        JOIN "User" u ON u.id = e."userId"
        WHERE e.scope = 'GLOBAL'::"LeaderboardScope"
          AND e.period = $1::"LeaderboardPeriod"
          AND e."snapshotDate" = $2
          AND u."isSuspended" = false
          AND (
            e."userId" = $3
            OR e."userId" IN (
                SELECT "followingId" FROM "Follow" WHERE "followerId" = $3
            )
          )
        ORDER BY e."returnPct" DESC, e."totalValue" DESC, e."userId" ASC
        "#,
    )
    .bind(period_sql)
    .bind(snapshot_date)
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::internal(format!("Failed to fetch friends leaderboard entries: {}", e)))?;

    let total = rows.len() as i64;
    let total_pages = if total == 0 {
        0
    } else {
        (total + limit_num - 1) / limit_num
    };

    let start = ((page_num - 1) * limit_num) as usize;
    let end = (start + limit_num as usize).min(rows.len());

    let items: Vec<LeaderboardEntryItem> = rows
        .into_iter()
        .enumerate()
        .map(|(index, r)| {
            let u_id: String = r.get("user_id");
            let user_name: String = r.get("user_name");
            let user_avatar_url: Option<String> = r.get("user_avatar_url");
            let total_value: Decimal = r.get("totalValue");
            let return_pct: Decimal = r.get("returnPct");
            let snap_date: NaiveDate = r.get("snapshotDate");
            let created_at: chrono::DateTime<Utc> = r.get("createdAt");

            LeaderboardEntryItem {
                rank: (index + 1) as i32,
                user_id: u_id.clone(),
                user: Some(LeaderboardUserSummary {
                    id: u_id,
                    name: user_name,
                    avatar_url: user_avatar_url,
                }),
                scope: LeaderboardScope::Global,
                period: period_enum,
                total_value: total_value.to_string(),
                return_pct: return_pct.to_string(),
                snapshot_date: snap_date,
                created_at,
            }
        })
        .skip(start)
        .take(end.saturating_sub(start))
        .collect();

    Ok(LeaderboardResponse {
        leaderboard: items,
        pagination: PaginationMeta {
            page: page_num,
            limit: limit_num,
            total,
            total_pages,
        },
    })
}

pub async fn get_leaderboard_history(
    pool: &PgPool,
    period_str: Option<&str>,
    date: Option<NaiveDate>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    page: Option<i64>,
    limit: Option<i64>,
) -> Result<LeaderboardResponse, AppError> {
    if date.is_some() && (from.is_some() || to.is_some()) {
        return Err(AppError::bad_request(
            "Cannot specify both exact date and date range (from/to)",
            "INVALID_QUERY",
        ));
    }
    if let (Some(f), Some(t)) = (from, to) {
        if f > t {
            return Err(AppError::bad_request(
                "from date must be before or equal to to date",
                "INVALID_QUERY",
            ));
        }
    }

    let (period_enum, period_sql) = parse_period(period_str);
    let page_num = page.unwrap_or(1).max(1);
    let limit_num = limit.unwrap_or(20).clamp(1, 100);
    let offset = (page_num - 1) * limit_num;

    let target_date = if date.is_none() && from.is_none() && to.is_none() {
        resolve_snapshot_date(pool, "GLOBAL", period_sql).await?
    } else {
        None
    };

    // Build dynamic query
    let mut count_query = String::from(
        r#"
        SELECT COUNT(*) as count
        FROM "LeaderboardEntry" e
        JOIN "User" u ON u.id = e."userId"
        WHERE e.scope = 'GLOBAL'::"LeaderboardScope"
          AND e.period = $1::"LeaderboardPeriod"
          AND u."isSuspended" = false
        "#,
    );

    let mut select_query = String::from(
        r#"
        SELECT
            e.id,
            e."userId",
            e.rank,
            e."totalValue",
            e."returnPct",
            e."snapshotDate",
            e."createdAt",
            u.id as user_id,
            u.name as user_name,
            u."avatarUrl" as user_avatar_url
        FROM "LeaderboardEntry" e
        JOIN "User" u ON u.id = e."userId"
        WHERE e.scope = 'GLOBAL'::"LeaderboardScope"
          AND e.period = $1::"LeaderboardPeriod"
          AND u."isSuspended" = false
        "#,
    );

    if let Some(d) = date {
        count_query.push_str(&format!(" AND e.\"snapshotDate\" = '{}'", d));
        select_query.push_str(&format!(" AND e.\"snapshotDate\" = '{}'", d));
        select_query.push_str(" ORDER BY e.rank ASC, e.\"userId\" ASC");
    } else if from.is_some() || to.is_some() {
        if let Some(f) = from {
            count_query.push_str(&format!(" AND e.\"snapshotDate\" >= '{}'", f));
            select_query.push_str(&format!(" AND e.\"snapshotDate\" >= '{}'", f));
        }
        if let Some(t) = to {
            count_query.push_str(&format!(" AND e.\"snapshotDate\" <= '{}'", t));
            select_query.push_str(&format!(" AND e.\"snapshotDate\" <= '{}'", t));
        }
        select_query.push_str(" ORDER BY e.\"snapshotDate\" DESC, e.rank ASC, e.\"userId\" ASC");
    } else if let Some(td) = target_date {
        count_query.push_str(&format!(" AND e.\"snapshotDate\" = '{}'", td));
        select_query.push_str(&format!(" AND e.\"snapshotDate\" = '{}'", td));
        select_query.push_str(" ORDER BY e.rank ASC, e.\"userId\" ASC");
    } else {
        return Ok(LeaderboardResponse {
            leaderboard: Vec::new(),
            pagination: PaginationMeta {
                page: page_num,
                limit: limit_num,
                total: 0,
                total_pages: 0,
            },
        });
    }

    select_query.push_str(&format!(" LIMIT {} OFFSET {}", limit_num, offset));

    let count_row = sqlx::query(&count_query)
        .bind(period_sql)
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to count history entries: {}", e)))?;

    let total: i64 = count_row.get("count");
    let total_pages = if total == 0 {
        0
    } else {
        (total + limit_num - 1) / limit_num
    };

    let rows = sqlx::query(&select_query)
        .bind(period_sql)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to fetch history entries: {}", e)))?;

    let items = rows
        .into_iter()
        .map(|r| {
            let u_id: String = r.get("user_id");
            let user_name: String = r.get("user_name");
            let user_avatar_url: Option<String> = r.get("user_avatar_url");
            let rank: i32 = r.get("rank");
            let total_value: Decimal = r.get("totalValue");
            let return_pct: Decimal = r.get("returnPct");
            let snap_date: NaiveDate = r.get("snapshotDate");
            let created_at: chrono::DateTime<Utc> = r.get("createdAt");

            LeaderboardEntryItem {
                rank,
                user_id: u_id.clone(),
                user: Some(LeaderboardUserSummary {
                    id: u_id,
                    name: user_name,
                    avatar_url: user_avatar_url,
                }),
                scope: LeaderboardScope::Global,
                period: period_enum,
                total_value: total_value.to_string(),
                return_pct: return_pct.to_string(),
                snapshot_date: snap_date,
                created_at,
            }
        })
        .collect();

    Ok(LeaderboardResponse {
        leaderboard: items,
        pagination: PaginationMeta {
            page: page_num,
            limit: limit_num,
            total,
            total_pages,
        },
    })
}
