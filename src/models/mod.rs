pub mod enums;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

pub use enums::*;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: String,
    pub email: String,
    #[serde(skip_serializing)]
    #[sqlx(rename = "passwordHash")]
    pub password_hash: Option<String>,
    #[sqlx(rename = "googleId")]
    pub google_id: Option<String>,
    pub name: String,
    #[sqlx(rename = "avatarUrl")]
    pub avatar_url: Option<String>,
    pub role: Role,
    #[sqlx(rename = "isVerified")]
    pub is_verified: bool,
    #[sqlx(rename = "isSuspended")]
    pub is_suspended: bool,
    #[sqlx(rename = "emailAlerts")]
    pub email_alerts: bool,
    #[sqlx(rename = "emailDigest")]
    pub email_digest: bool,
    #[sqlx(rename = "pushEnabled")]
    pub push_enabled: bool,
    #[sqlx(rename = "pushToken")]
    pub push_token: Option<String>,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
    #[sqlx(rename = "lastLoginAt")]
    pub last_login_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizedUser {
    pub id: String,
    pub email: String,
    pub name: String,
    #[serde(rename = "avatarUrl")]
    pub avatar_url: Option<String>,
    pub role: Role,
    #[serde(rename = "isVerified")]
    pub is_verified: bool,
    #[serde(rename = "isSuspended")]
    pub is_suspended: bool,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
    #[serde(rename = "lastLoginAt")]
    pub last_login_at: Option<DateTime<Utc>>,
}

impl From<User> for SanitizedUser {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            email: u.email,
            name: u.name,
            avatar_url: u.avatar_url,
            role: u.role,
            is_verified: u.is_verified,
            is_suspended: u.is_suspended,
            created_at: u.created_at,
            updated_at: u.updated_at,
            last_login_at: u.last_login_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RefreshToken {
    pub id: String,
    pub token: String,
    #[sqlx(rename = "userId")]
    pub user_id: String,
    #[sqlx(rename = "expiresAt")]
    pub expires_at: DateTime<Utc>,
    #[sqlx(rename = "revokedAt")]
    pub revoked_at: Option<DateTime<Utc>>,
    #[sqlx(rename = "userAgent")]
    pub user_agent: Option<String>,
    #[sqlx(rename = "ipAddress")]
    pub ip_address: Option<String>,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserPreference {
    pub id: String,
    #[sqlx(rename = "userId")]
    pub user_id: String,
    #[sqlx(rename = "emailNotifications")]
    pub email_notifications: bool,
    #[sqlx(rename = "pushNotifications")]
    pub push_notifications: bool,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Portfolio {
    pub id: String,
    #[sqlx(rename = "userId")]
    pub user_id: String,
    pub name: String,
    pub description: Option<String>,
    #[sqlx(rename = "startingBalance")]
    pub starting_balance: Decimal,
    #[sqlx(rename = "cashBalance")]
    pub cash_balance: Decimal,
    #[sqlx(rename = "reservedCash")]
    pub reserved_cash: Decimal,
    #[sqlx(rename = "isDefault")]
    pub is_default: bool,
    #[sqlx(rename = "isActive")]
    pub is_active: bool,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Holding {
    pub id: String,
    #[sqlx(rename = "portfolioId")]
    pub portfolio_id: String,
    pub symbol: String,
    #[sqlx(rename = "assetType")]
    pub asset_type: AssetType,
    pub quantity: Decimal,
    #[sqlx(rename = "averageBuyPrice")]
    pub average_buy_price: Decimal,
    #[sqlx(rename = "totalInvested")]
    pub total_invested: Decimal,
    #[sqlx(rename = "isShort")]
    pub is_short: bool,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Order {
    pub id: String,
    #[sqlx(rename = "portfolioId")]
    pub portfolio_id: String,
    pub symbol: String,
    #[sqlx(rename = "assetType")]
    pub asset_type: AssetType,
    pub side: OrderSide,
    #[sqlx(rename = "type")]
    pub order_type: OrderType,
    pub status: OrderStatus,
    pub quantity: Decimal,
    #[sqlx(rename = "reservedCash")]
    pub reserved_cash: Decimal,
    #[sqlx(rename = "limitPrice")]
    pub limit_price: Option<Decimal>,
    #[sqlx(rename = "stopPrice")]
    pub stop_price: Option<Decimal>,
    #[sqlx(rename = "executedPrice")]
    pub executed_price: Option<Decimal>,
    #[sqlx(rename = "executedAt")]
    pub executed_at: Option<DateTime<Utc>>,
    #[sqlx(rename = "expiresAt")]
    pub expires_at: Option<DateTime<Utc>>,
    #[sqlx(rename = "executionAttempts")]
    pub execution_attempts: i32,
    #[sqlx(rename = "lastExecutionAttemptAt")]
    pub last_execution_attempt_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub source: OrderSource,
    #[sqlx(rename = "copyTradeExecutionId")]
    pub copy_trade_execution_id: Option<String>,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OrderEvent {
    pub id: String,
    #[sqlx(rename = "orderId")]
    pub order_id: String,
    pub sequence: i32,
    #[sqlx(rename = "eventType")]
    pub event_type: OrderEventType,
    #[sqlx(rename = "previousStatus")]
    pub previous_status: Option<OrderStatus>,
    #[sqlx(rename = "newStatus")]
    pub new_status: OrderStatus,
    pub metadata: Option<serde_json::Value>,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Trade {
    pub id: String,
    #[sqlx(rename = "portfolioId")]
    pub portfolio_id: String,
    #[sqlx(rename = "orderId")]
    pub order_id: String,
    pub symbol: String,
    #[sqlx(rename = "assetType")]
    pub asset_type: AssetType,
    pub side: OrderSide,
    pub quantity: Decimal,
    pub price: Decimal,
    #[sqlx(rename = "totalValue")]
    pub total_value: Decimal,
    pub fees: Decimal,
    #[sqlx(rename = "realizedPnl")]
    pub realized_pnl: Option<Decimal>,
    #[sqlx(rename = "realizedPnlPct")]
    pub realized_pnl_pct: Option<Decimal>,
    #[sqlx(rename = "executedAt")]
    pub executed_at: DateTime<Utc>,
    #[sqlx(rename = "copyFanOutAt")]
    pub copy_fan_out_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PerformanceSnapshot {
    pub id: String,
    #[sqlx(rename = "portfolioId")]
    pub portfolio_id: String,
    #[sqlx(rename = "totalValue")]
    pub total_value: Decimal,
    #[sqlx(rename = "cashBalance")]
    pub cash_balance: Decimal,
    #[sqlx(rename = "holdingsValue")]
    pub holdings_value: Decimal,
    #[sqlx(rename = "dailyPnl")]
    pub daily_pnl: Decimal,
    #[sqlx(rename = "dailyPnlPct")]
    pub daily_pnl_pct: Decimal,
    #[sqlx(rename = "snapshotDate")]
    pub snapshot_date: NaiveDate,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Notification {
    pub id: String,
    #[sqlx(rename = "eventKey")]
    pub event_key: Option<String>,
    #[sqlx(rename = "userId")]
    pub user_id: String,
    #[sqlx(rename = "type")]
    pub notification_type: NotificationType,
    pub title: String,
    pub message: String,
    pub data: Option<serde_json::Value>,
    #[sqlx(rename = "isRead")]
    pub is_read: bool,
    #[sqlx(rename = "readAt")]
    pub read_at: Option<DateTime<Utc>>,
    #[sqlx(rename = "deliveryStatus")]
    pub delivery_status: NotificationDeliveryStatus,
    #[sqlx(rename = "deliveredAt")]
    pub delivered_at: Option<DateTime<Utc>>,
    #[sqlx(rename = "deliveryAttempts")]
    pub delivery_attempts: i32,
    #[sqlx(rename = "deliveryStartedAt")]
    pub delivery_started_at: Option<DateTime<Utc>>,
    #[sqlx(rename = "deliveryChannels")]
    pub delivery_channels: Option<serde_json::Value>,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Alert {
    pub id: String,
    #[sqlx(rename = "userId")]
    pub user_id: String,
    #[sqlx(rename = "symbolId")]
    pub symbol_id: String,
    pub condition: AlertCondition,
    #[sqlx(rename = "targetPrice")]
    pub target_price: Decimal,
    pub status: AlertStatus,
    #[sqlx(rename = "sendEmail")]
    pub send_email: bool,
    #[sqlx(rename = "sendPush")]
    pub send_push: bool,
    #[sqlx(rename = "triggeredAt")]
    pub triggered_at: Option<DateTime<Utc>>,
    pub message: Option<String>,
    #[sqlx(rename = "baselinePrice")]
    pub baseline_price: Option<Decimal>,
    #[sqlx(rename = "baselineAt")]
    pub baseline_at: Option<DateTime<Utc>>,
    #[sqlx(rename = "lastEvaluatedAt")]
    pub last_evaluated_at: Option<DateTime<Utc>>,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SymbolModel {
    pub id: String,
    pub symbol: String,
    pub name: String,
    #[sqlx(rename = "assetType")]
    pub asset_type: AssetType,
    pub exchange: Option<String>,
    pub currency: String,
    pub sector: Option<String>,
    pub industry: Option<String>,
    #[sqlx(rename = "logoUrl")]
    pub logo_url: Option<String>,
    pub description: Option<String>,
    #[sqlx(rename = "isActive")]
    pub is_active: bool,
    #[sqlx(rename = "isTradable")]
    pub is_tradable: bool,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Follow {
    pub id: String,
    #[sqlx(rename = "followerId")]
    pub follower_id: String,
    #[sqlx(rename = "followingId")]
    pub following_id: String,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CopyTrader {
    pub id: String,
    #[sqlx(rename = "copierId")]
    pub copier_id: String,
    #[sqlx(rename = "copiedId")]
    pub copied_id: String,
    #[sqlx(rename = "copyPortfolioId")]
    pub copy_portfolio_id: Option<String>,
    #[sqlx(rename = "allocationPct")]
    pub allocation_pct: Decimal,
    #[sqlx(rename = "maxTradeSize")]
    pub max_trade_size: Option<Decimal>,
    #[sqlx(rename = "isActive")]
    pub is_active: bool,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CopyTradeExecution {
    pub id: String,
    #[sqlx(rename = "sourceTradeId")]
    pub source_trade_id: String,
    #[sqlx(rename = "copierId")]
    pub copier_id: String,
    #[sqlx(rename = "copyTraderId")]
    pub copy_trader_id: Option<String>,
    pub status: CopyExecutionStatus,
    pub attempts: i32,
    #[sqlx(rename = "failureCode")]
    pub failure_code: Option<String>,
    #[sqlx(rename = "failureMessage")]
    pub failure_message: Option<String>,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Post {
    pub id: String,
    #[sqlx(rename = "authorId")]
    pub author_id: String,
    pub content: String,
    pub symbol: Option<String>,
    pub metadata: Option<serde_json::Value>,
    #[sqlx(rename = "isHidden")]
    pub is_hidden: bool,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PostLike {
    pub id: String,
    #[sqlx(rename = "postId")]
    pub post_id: String,
    #[sqlx(rename = "userId")]
    pub user_id: String,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SentimentVote {
    pub id: String,
    #[sqlx(rename = "userId")]
    pub user_id: String,
    pub symbol: String,
    pub sentiment: Sentiment,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Achievement {
    pub id: String,
    pub code: String,
    pub name: String,
    pub description: String,
    #[sqlx(rename = "iconUrl")]
    pub icon_url: Option<String>,
    pub category: String,
    pub criteria: Option<serde_json::Value>,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserAchievement {
    pub id: String,
    #[sqlx(rename = "userId")]
    pub user_id: String,
    #[sqlx(rename = "achievementId")]
    pub achievement_id: String,
    pub reason: String,
    #[sqlx(rename = "unlockedAt")]
    pub unlocked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct League {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[sqlx(rename = "startingBalance")]
    pub starting_balance: Decimal,
    #[sqlx(rename = "maxParticipants")]
    pub max_participants: Option<i32>,
    #[sqlx(rename = "startDate")]
    pub start_date: DateTime<Utc>,
    #[sqlx(rename = "endDate")]
    pub end_date: DateTime<Utc>,
    pub status: LeagueStatus,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LeagueMember {
    pub id: String,
    #[sqlx(rename = "leagueId")]
    pub league_id: String,
    #[sqlx(rename = "userId")]
    pub user_id: String,
    #[sqlx(rename = "portfolioId")]
    pub portfolio_id: String,
    pub rank: Option<i32>,
    #[sqlx(rename = "finalReturn")]
    pub final_return: Option<Decimal>,
    #[sqlx(rename = "joinedAt")]
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LeaderboardSnapshot {
    pub id: String,
    pub scope: LeaderboardScope,
    pub period: LeaderboardPeriod,
    #[sqlx(rename = "snapshotDate")]
    pub snapshot_date: NaiveDate,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LeaderboardEntry {
    pub id: String,
    #[sqlx(rename = "userId")]
    pub user_id: String,
    pub scope: LeaderboardScope,
    pub period: LeaderboardPeriod,
    pub rank: i32,
    #[sqlx(rename = "totalValue")]
    pub total_value: Decimal,
    #[sqlx(rename = "returnPct")]
    pub return_pct: Decimal,
    #[sqlx(rename = "snapshotDate")]
    pub snapshot_date: NaiveDate,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "snapshotId")]
    pub snapshot_id: String,
}
