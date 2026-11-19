use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, Mutex};
use sqlx::PgPool;

use crate::config::AppConfig;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RealtimeMessage {
    pub channel: String,
    pub event: String,
    pub payload: serde_json::Value,
}

pub struct RateLimiter {
    // Map of key -> Vec of hit timestamps
    records: Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
        }
    }

    pub async fn check_and_record(
        &self,
        key: &str,
        window: std::time::Duration,
        max_requests: u32,
    ) -> bool {
        let mut map = self.records.lock().await;
        let now = Instant::now();
        let cutoff = now.checked_sub(window).unwrap_or(now);

        let entry = map.entry(key.to_string()).or_default();
        entry.retain(|&time| time > cutoff);

        if entry.len() >= max_requests as usize {
            false
        } else {
            entry.push(now);
            true
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<AppConfig>,
    pub http_client: reqwest::Client,
    pub redis_client: Option<redis::Client>,
    pub rate_limiter: Arc<RateLimiter>,
    pub tx_realtime: broadcast::Sender<RealtimeMessage>,
}

impl AppState {
    pub fn new(pool: PgPool, config: Arc<AppConfig>, redis_client: Option<redis::Client>) -> Self {
        let (tx_realtime, _) = broadcast::channel(1024);
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(5000))
            .build()
            .unwrap_or_default();

        Self {
            pool,
            config,
            http_client,
            redis_client,
            rate_limiter: Arc::new(RateLimiter::new()),
            tx_realtime,
        }
    }
}
