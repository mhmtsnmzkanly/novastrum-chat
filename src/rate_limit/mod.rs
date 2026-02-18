use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Runtime configurable rate limits (normally sourced from DB table `rate_config`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateConfig {
    /// Minimum interval between messages for one user.
    pub message_every_seconds: i64,
    /// Minimum interval between upvotes for one user.
    pub upvote_every_seconds: i64,
    /// DM request limit per 1 minute window.
    pub dm_per_minute: usize,
    /// DM request limit per 1 hour window.
    pub dm_per_hour: usize,
    /// DM request limit per 1 day window.
    pub dm_per_day: usize,
}

impl Default for RateConfig {
    /// Builds spec-aligned fallback values.
    fn default() -> Self {
        Self {
            message_every_seconds: 3,
            upvote_every_seconds: 60,
            dm_per_minute: 2,
            dm_per_hour: 10,
            dm_per_day: 30,
        }
    }
}

/// Login failure state used for progressive lockout timings.
#[derive(Debug, Clone, Default)]
struct LoginFailureState {
    /// Consecutive failed attempts for one login name.
    consecutive_failures: u32,
    /// Optional lock expiration after failures.
    lock_until: Option<DateTime<Utc>>,
}

/// Internal mutable data for all rate buckets.
#[derive(Debug, Default)]
struct RateLimitState {
    /// Login failure records keyed by normalized username.
    login_failures: HashMap<String, LoginFailureState>,
    /// Last message timestamp per user.
    message_last_sent_at: HashMap<Uuid, DateTime<Utc>>,
    /// Last vote timestamp per user.
    vote_last_cast_at: HashMap<Uuid, DateTime<Utc>>,
    /// DM request timestamps per user.
    dm_request_timestamps: HashMap<Uuid, Vec<DateTime<Utc>>>,
}

/// Thread-safe rate limiter service.
#[derive(Debug, Clone)]
pub struct RateLimitService {
    /// Configuration values controlling thresholds.
    config: Arc<RwLock<RateConfig>>,
    /// Lock-protected mutable state.
    state: Arc<RwLock<RateLimitState>>,
}

impl RateLimitService {
    /// Creates a new service with spec defaults.
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(RateConfig::default())),
            state: Arc::new(RwLock::new(RateLimitState::default())),
        }
    }

    /// Replaces current runtime limits with values loaded from storage.
    pub async fn set_config(&self, config: RateConfig) {
        let mut guard = self.config.write().await;
        *guard = config;
    }

    /// Returns current runtime limits for admin diagnostics/edit screens.
    pub async fn get_config(&self) -> RateConfig {
        self.config.read().await.clone()
    }

    /// Returns a snapshot of current rate configuration.
    async fn config_snapshot(&self) -> RateConfig {
        self.config.read().await.clone()
    }

    /// Validates whether login attempt is currently blocked by lockout policy.
    pub async fn can_attempt_login(
        &self,
        normalized_login: &str,
        now: DateTime<Utc>,
    ) -> Result<(), i64> {
        let guard = self.state.read().await;
        let Some(entry) = guard.login_failures.get(normalized_login) else {
            return Ok(());
        };

        // If lock is active we return remaining seconds so caller can show proper message.
        if let Some(lock_until) = entry.lock_until
            && lock_until > now
        {
            return Err((lock_until - now).num_seconds().max(1));
        }

        Ok(())
    }

    /// Records failed login and returns lock duration in seconds (0 when not locked).
    pub async fn record_login_failure(&self, normalized_login: &str, now: DateTime<Utc>) -> i64 {
        let mut guard = self.state.write().await;
        let entry = guard
            .login_failures
            .entry(normalized_login.to_string())
            .or_default();

        // Increase consecutive failure count.
        entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);

        // Spec rule: first 3 failures -> 3 minutes lock, then +30s each additional fail.
        if entry.consecutive_failures >= 3 {
            let additional_failures = entry.consecutive_failures.saturating_sub(3);
            let lock_seconds = 180 + i64::from(additional_failures) * 30;
            entry.lock_until = Some(now + Duration::seconds(lock_seconds));
            return lock_seconds;
        }

        0
    }

    /// Resets login failure tracking after successful authentication.
    pub async fn clear_login_failures(&self, normalized_login: &str) {
        let mut guard = self.state.write().await;
        guard.login_failures.remove(normalized_login);
    }

    /// Enforces `1 message / 3 sec` user-based limit.
    pub async fn allow_message(&self, user_id: Uuid, now: DateTime<Utc>) -> bool {
        let mut guard = self.state.write().await;
        let config = self.config_snapshot().await;
        let min_wait = Duration::seconds(config.message_every_seconds);

        if let Some(last_sent) = guard.message_last_sent_at.get(&user_id)
            && (*last_sent + min_wait) > now
        {
            return false;
        }

        guard.message_last_sent_at.insert(user_id, now);
        true
    }

    /// Enforces `1 vote / minute` user-based limit.
    pub async fn allow_vote(&self, user_id: Uuid, now: DateTime<Utc>) -> bool {
        let mut guard = self.state.write().await;
        let config = self.config_snapshot().await;
        let min_wait = Duration::seconds(config.upvote_every_seconds);

        if let Some(last_vote) = guard.vote_last_cast_at.get(&user_id)
            && (*last_vote + min_wait) > now
        {
            return false;
        }

        guard.vote_last_cast_at.insert(user_id, now);
        true
    }

    /// Enforces DM start limits (2/min, 10/hour, 30/day) per user.
    pub async fn allow_dm_request(&self, user_id: Uuid, now: DateTime<Utc>) -> bool {
        let mut guard = self.state.write().await;
        let config = self.config_snapshot().await;
        let timestamps = guard.dm_request_timestamps.entry(user_id).or_default();

        // Keep only last 24h window because older entries are irrelevant.
        let day_threshold = now - Duration::hours(24);
        timestamps.retain(|ts| *ts >= day_threshold);

        // Compute rolling-window counters from retained timestamps.
        let minute_threshold = now - Duration::minutes(1);
        let hour_threshold = now - Duration::hours(1);

        let in_minute = timestamps
            .iter()
            .filter(|ts| **ts >= minute_threshold)
            .count();
        let in_hour = timestamps
            .iter()
            .filter(|ts| **ts >= hour_threshold)
            .count();
        let in_day = timestamps.len();

        // Reject when any configured DM window would be exceeded.
        if in_minute >= config.dm_per_minute
            || in_hour >= config.dm_per_hour
            || in_day >= config.dm_per_day
        {
            return false;
        }

        // Accept and store the new timestamp.
        timestamps.push(now);
        true
    }
}
