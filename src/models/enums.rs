use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "Role", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Role {
    Trader,
    Admin,
    Moderator,
}

impl Default for Role {
    fn default() -> Self {
        Role::Trader
    }
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Trader => "TRADER",
            Role::Admin => "ADMIN",
            Role::Moderator => "MODERATOR",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "AssetType", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssetType {
    Stock,
    Crypto,
    Forex,
}

impl AssetType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AssetType::Stock => "STOCK",
            AssetType::Crypto => "CRYPTO",
            AssetType::Forex => "FOREX",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "OrderSide", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "OrderType", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderType {
    Market,
    Limit,
    StopLoss,
    TakeProfit,
}

impl OrderType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderType::Market => "MARKET",
            OrderType::Limit => "LIMIT",
            OrderType::StopLoss => "STOP_LOSS",
            OrderType::TakeProfit => "TAKE_PROFIT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "OrderStatus", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    Pending,
    Executed,
    Cancelled,
    Expired,
    Rejected,
}

impl OrderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderStatus::Pending => "PENDING",
            OrderStatus::Executed => "EXECUTED",
            OrderStatus::Cancelled => "CANCELLED",
            OrderStatus::Expired => "EXPIRED",
            OrderStatus::Rejected => "REJECTED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "OrderEventType", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderEventType {
    Created,
    Executed,
    Cancelled,
    Expired,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "OrderSource", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderSource {
    Manual,
    CopyTrade,
    DcaBot,
    League,
}

impl Default for OrderSource {
    fn default() -> Self {
        OrderSource::Manual
    }
}

impl OrderSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderSource::Manual => "MANUAL",
            OrderSource::CopyTrade => "COPY_TRADE",
            OrderSource::DcaBot => "DCA_BOT",
            OrderSource::League => "LEAGUE",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "AlertCondition", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlertCondition {
    PriceAbove,
    PriceBelow,
    PercentChangeUp,
    PercentChangeDown,
}

impl AlertCondition {
    pub fn as_str(&self) -> &'static str {
        match self {
            AlertCondition::PriceAbove => "PRICE_ABOVE",
            AlertCondition::PriceBelow => "PRICE_BELOW",
            AlertCondition::PercentChangeUp => "PERCENT_CHANGE_UP",
            AlertCondition::PercentChangeDown => "PERCENT_CHANGE_DOWN",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "AlertStatus", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlertStatus {
    Active,
    Triggered,
    Expired,
    Cancelled,
}

impl Default for AlertStatus {
    fn default() -> Self {
        AlertStatus::Active
    }
}

impl AlertStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            AlertStatus::Active => "ACTIVE",
            AlertStatus::Triggered => "TRIGGERED",
            AlertStatus::Expired => "EXPIRED",
            AlertStatus::Cancelled => "CANCELLED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "NotificationType", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotificationType {
    OrderExecuted,
    OrderCancelled,
    OrderRejected,
    OrderExpired,
    PriceAlert,
    AchievementUnlocked,
    CopyTradeExecuted,
    LeagueUpdate,
    NewFollower,
    System,
}

impl NotificationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            NotificationType::OrderExecuted => "ORDER_EXECUTED",
            NotificationType::OrderCancelled => "ORDER_CANCELLED",
            NotificationType::OrderRejected => "ORDER_REJECTED",
            NotificationType::OrderExpired => "ORDER_EXPIRED",
            NotificationType::PriceAlert => "PRICE_ALERT",
            NotificationType::AchievementUnlocked => "ACHIEVEMENT_UNLOCKED",
            NotificationType::CopyTradeExecuted => "COPY_TRADE_EXECUTED",
            NotificationType::LeagueUpdate => "LEAGUE_UPDATE",
            NotificationType::NewFollower => "NEW_FOLLOWER",
            NotificationType::System => "SYSTEM",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "NotificationDeliveryStatus", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotificationDeliveryStatus {
    Pending,
    Sending,
    Delivered,
    Skipped,
    Failed,
}

impl Default for NotificationDeliveryStatus {
    fn default() -> Self {
        NotificationDeliveryStatus::Pending
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "LeaderboardScope", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LeaderboardScope {
    Global,
    Friends,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "LeaderboardPeriod", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LeaderboardPeriod {
    Daily,
    Weekly,
    Monthly,
    AllTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "LeagueStatus", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LeagueStatus {
    Upcoming,
    Active,
    Completed,
    Cancelled,
}

impl Default for LeagueStatus {
    fn default() -> Self {
        LeagueStatus::Upcoming
    }
}

impl LeagueStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            LeagueStatus::Upcoming => "UPCOMING",
            LeagueStatus::Active => "ACTIVE",
            LeagueStatus::Completed => "COMPLETED",
            LeagueStatus::Cancelled => "CANCELLED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "Sentiment", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Sentiment {
    Bullish,
    Bearish,
    Neutral,
}

impl Sentiment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Sentiment::Bullish => "BULLISH",
            Sentiment::Bearish => "BEARISH",
            Sentiment::Neutral => "NEUTRAL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "CopyExecutionStatus", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CopyExecutionStatus {
    Pending,
    Placed,
    Failed,
    Skipped,
}

impl Default for CopyExecutionStatus {
    fn default() -> Self {
        CopyExecutionStatus::Pending
    }
}
