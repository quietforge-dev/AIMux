use std::collections::HashMap;

use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::{config::Settings, dao::account_dao, error::AppError};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Policy(bool, u32, u8, u8);

impl Policy {
    fn from_settings(settings: &Settings) -> Self {
        Self(
            settings.low_cache_priority_enabled,
            settings.low_cache_consecutive,
            settings.low_cache_threshold_percent,
            settings.low_cache_priority_penalty,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings {
            low_cache_priority_enabled: true,
            ..Settings::default()
        }
    }

    async fn pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE accounts (id TEXT PRIMARY KEY, priority INTEGER, last_error_code TEXT, last_error_message TEXT, updated_at TEXT)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO accounts (id,priority) VALUES ('a',9),('b',9)")
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    async fn priority(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT priority FROM accounts WHERE id='a'")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn slow_only_deducts_every_five_without_bonus_and_is_account_scoped() {
        let pool = pool().await;
        let settings = Settings {
            slow_first_token_enabled: true,
            ..Settings::default()
        };
        let mut tracker = CachePriorityTracker::new(&settings);
        for n in 1..=10 {
            let ticket = tracker.begin("a", &settings).unwrap();
            tracker
                .finish_success(&pool, "a", Some(&ticket), None, None, Some(10001))
                .await
                .unwrap();
            let b = tracker.begin("b", &settings).unwrap();
            tracker
                .finish_success(&pool, "b", Some(&b), None, None, Some(10000))
                .await
                .unwrap();
            assert_eq!(priority(&pool).await, 9 - (n / 5) * 2);
            assert_eq!(tracker.accounts["a"].slow_count, (n % 5) as u32);
            assert_eq!(tracker.accounts["b"].slow_count, 0);
        }
    }

    #[tokio::test]
    async fn slow_threshold_missing_nonstream_and_failures_reset_only_as_expected() {
        let pool = pool().await;
        let settings = Settings {
            slow_first_token_enabled: true,
            ..settings()
        };
        let mut tracker = CachePriorityTracker::new(&settings);
        for ms in [Some(10000), Some(0), Some(-1), None] {
            let ticket = tracker.begin("a", &settings).unwrap();
            tracker
                .finish_success(&pool, "a", Some(&ticket), Some(100), Some(0), Some(11000))
                .await
                .unwrap();
            tracker
                .finish_success(&pool, "a", Some(&ticket), Some(100), Some(0), ms)
                .await
                .unwrap();
            assert_eq!(tracker.accounts["a"].slow_count, 0);
            tracker.reset(Some(&ticket));
            assert_eq!(tracker.accounts["a"].count, 0);
        }
        let ticket = tracker.begin("a", &settings).unwrap();
        tracker
            .finish_success(&pool, "a", Some(&ticket), Some(100), Some(0), Some(11000))
            .await
            .unwrap();
        tracker
            .finish_success(&pool, "a", Some(&ticket), Some(100), Some(85), Some(11000))
            .await
            .unwrap();
        assert_eq!(tracker.accounts["a"].count, 0);
        assert_eq!(tracker.accounts["a"].slow_count, 2);
        tracker
            .finish_success(&pool, "a", Some(&ticket), Some(100), Some(0), Some(10000))
            .await
            .unwrap();
        assert_eq!(tracker.accounts["a"].count, 1);
        assert_eq!(tracker.accounts["a"].slow_count, 0);
        tracker.reset(Some(&ticket));
        assert_eq!(tracker.accounts["a"].count, 0);
    }

    #[tokio::test]
    async fn changing_one_policy_preserves_other_streak_and_rejects_stale_policy() {
        let pool = pool().await;
        let mut settings = Settings {
            slow_first_token_enabled: true,
            ..settings()
        };
        let mut tracker = CachePriorityTracker::new(&settings);
        let old = tracker.begin("a", &settings).unwrap();
        tracker
            .finish_success(&pool, "a", Some(&old), Some(100), Some(0), Some(11000))
            .await
            .unwrap();
        settings.slow_first_token_threshold_seconds = 12;
        tracker.configure(&settings);
        assert_eq!(tracker.accounts["a"].count, 1);
        assert_eq!(tracker.accounts["a"].slow_count, 0);
        tracker
            .finish_success(&pool, "a", Some(&old), Some(100), Some(0), Some(13000))
            .await
            .unwrap();
        assert_eq!(tracker.accounts["a"].count, 2);
        assert_eq!(tracker.accounts["a"].slow_count, 0);
        let fresh = tracker.begin("a", &settings).unwrap();
        tracker
            .finish_success(&pool, "a", Some(&fresh), Some(100), Some(0), Some(13000))
            .await
            .unwrap();
        settings.low_cache_threshold_percent = 90;
        tracker.configure(&settings);
        assert_eq!(tracker.accounts["a"].count, 0);
        assert_eq!(tracker.accounts["a"].slow_count, 1);
        tracker.reset(Some(&old));
        assert_eq!(tracker.accounts["a"].slow_count, 1);
        settings.slow_first_token_enabled = false;
        tracker.configure(&settings);
        assert_eq!(tracker.accounts["a"].slow_count, 0);
        tracker.remove("a");
        let fresh = tracker.begin("a", &settings).unwrap();
        tracker
            .finish_success(&pool, "a", Some(&old), Some(100), Some(0), Some(13000))
            .await
            .unwrap();
        assert_eq!(tracker.accounts["a"].count, 0);
        assert!(fresh.slow_epoch.is_none());
    }

    #[tokio::test]
    async fn concurrent_combined_triggers_use_single_sum_and_respect_zero_floor() {
        let pool = pool().await;
        let settings = Settings {
            slow_first_token_enabled: true,
            ..settings()
        };
        let tracker = std::sync::Arc::new(Mutex::new(CachePriorityTracker::new(&settings)));
        let mut tasks = Vec::new();
        for _ in 0..10 {
            let ticket = tracker.lock().await.begin("a", &settings).unwrap();
            let tracker = tracker.clone();
            let pool = pool.clone();
            tasks.push(tokio::spawn(async move {
                tracker
                    .lock()
                    .await
                    .finish_success(&pool, "a", Some(&ticket), Some(100), Some(0), Some(11000))
                    .await
                    .unwrap();
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(priority(&pool).await, 0);
        let tracker = tracker.lock().await;
        assert_eq!(tracker.accounts["a"].count, 0);
        assert_eq!(tracker.accounts["a"].slow_count, 0);
    }

    #[tokio::test]
    async fn combined_trigger_deducts_five_and_failed_write_preserves_both_streaks() {
        let pool = pool().await;
        let settings = Settings {
            slow_first_token_enabled: true,
            ..settings()
        };
        let mut tracker = CachePriorityTracker::new(&settings);
        let ticket = tracker.begin("a", &settings).unwrap();
        for _ in 0..4 {
            tracker
                .finish_success(&pool, "a", Some(&ticket), Some(100), Some(0), Some(11000))
                .await
                .unwrap();
        }
        assert_eq!(priority(&pool).await, 9);
        sqlx::query("CREATE TRIGGER reject_update BEFORE UPDATE ON accounts BEGIN SELECT RAISE(FAIL, 'test'); END")
            .execute(&pool).await.unwrap();
        assert!(tracker
            .finish_success(&pool, "a", Some(&ticket), Some(100), Some(0), Some(11000))
            .await
            .is_err());
        assert_eq!(tracker.accounts["a"].count, 4);
        assert_eq!(tracker.accounts["a"].slow_count, 4);
        sqlx::query("DROP TRIGGER reject_update")
            .execute(&pool)
            .await
            .unwrap();
        tracker
            .finish_success(&pool, "a", Some(&ticket), Some(100), Some(0), Some(11000))
            .await
            .unwrap();
        assert_eq!(priority(&pool).await, 4);
        assert_eq!(tracker.accounts["a"].count, 0);
        assert_eq!(tracker.accounts["a"].slow_count, 0);
    }

    #[tokio::test]
    async fn counts_per_account_and_deducts_once_per_group_without_success_bonus() {
        let pool = pool().await;
        let settings = settings();
        let mut tracker = CachePriorityTracker::new(&settings);
        for n in 1..=10 {
            let a = tracker.begin("a", &settings).unwrap();
            tracker
                .success(&pool, "a", Some(&a), Some(100), Some(84))
                .await
                .unwrap();
            let b = tracker.begin("b", &settings).unwrap();
            tracker
                .success(&pool, "b", Some(&b), Some(100), Some(85))
                .await
                .unwrap();
            assert_eq!(priority(&pool).await, 9 - (n / 5) * 3);
            assert_eq!(tracker.accounts["a"].count, (n % 5) as u32);
        }
    }

    #[tokio::test]
    async fn threshold_missing_and_invalid_usage_break_streak_but_zero_cache_is_valid() {
        let pool = pool().await;
        let settings = settings();
        let mut tracker = CachePriorityTracker::new(&settings);
        for (input, cached) in [
            (Some(100), Some(85)),
            (Some(100), None),
            (Some(0), Some(0)),
            (Some(100), Some(-1)),
            (Some(100), Some(101)),
            (None, Some(0)),
        ] {
            let ticket = tracker.begin("a", &settings).unwrap();
            tracker
                .success(&pool, "a", Some(&ticket), Some(100), Some(0))
                .await
                .unwrap();
            assert_eq!(tracker.accounts["a"].count, 1);
            tracker
                .success(&pool, "a", Some(&ticket), input, cached)
                .await
                .unwrap();
            assert_eq!(tracker.accounts["a"].count, 0);
        }
        let ticket = tracker.begin("a", &settings).unwrap();
        tracker
            .success(&pool, "a", Some(&ticket), Some(100), Some(0))
            .await
            .unwrap();
        tracker.reset(Some(&ticket));
        assert_eq!(tracker.accounts["a"].count, 0);
    }

    #[tokio::test]
    async fn reconfigure_disable_and_account_removal_invalidate_inflight_tickets() {
        let pool = pool().await;
        let mut settings = settings();
        let mut tracker = CachePriorityTracker::new(&settings);
        let old = tracker.begin("a", &settings).unwrap();
        tracker
            .success(&pool, "a", Some(&old), Some(100), Some(0))
            .await
            .unwrap();
        // Unrelated settings preserve the streak.
        settings.request_retry_attempts = 3;
        tracker.configure(&settings);
        assert_eq!(tracker.accounts["a"].count, 1);
        settings.low_cache_threshold_percent = 90;
        tracker.configure(&settings);
        let fresh = tracker.begin("a", &settings).unwrap();
        tracker
            .success(&pool, "a", Some(&fresh), Some(100), Some(0))
            .await
            .unwrap();
        tracker
            .success(&pool, "a", Some(&old), Some(100), Some(0))
            .await
            .unwrap();
        tracker.reset(Some(&old));
        assert_eq!(tracker.accounts["a"].count, 1);
        tracker.remove("a");
        let after_edit = tracker.begin("a", &settings).unwrap();
        tracker
            .success(&pool, "a", Some(&fresh), Some(100), Some(0))
            .await
            .unwrap();
        assert_eq!(tracker.accounts["a"].count, 0);
        tracker
            .success(&pool, "a", Some(&after_edit), Some(100), Some(0))
            .await
            .unwrap();
        settings.low_cache_priority_enabled = false;
        tracker.configure(&settings);
        assert!(tracker.accounts.is_empty());
        assert!(tracker.begin("a", &settings).is_none());
        tracker
            .success(&pool, "a", None, Some(100), Some(0))
            .await
            .unwrap();
        assert!(tracker.accounts.is_empty());
        assert!(CachePriorityTracker::new(&settings).accounts.is_empty());
        let mut old_settings = settings.clone();
        old_settings.low_cache_priority_enabled = true;
        assert!(tracker.begin("a", &old_settings).is_none());
    }

    #[tokio::test]
    async fn concurrent_completions_trigger_exactly_two_deductions() {
        let pool = pool().await;
        let settings = settings();
        let tracker = std::sync::Arc::new(Mutex::new(CachePriorityTracker::new(&settings)));
        let mut tasks = Vec::new();
        for _ in 0..10 {
            let ticket = tracker.lock().await.begin("a", &settings).unwrap();
            let tracker = tracker.clone();
            let pool = pool.clone();
            tasks.push(tokio::spawn(async move {
                tracker
                    .lock()
                    .await
                    .success(&pool, "a", Some(&ticket), Some(100), Some(0))
                    .await
                    .unwrap();
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(priority(&pool).await, 3);
        assert_eq!(tracker.lock().await.accounts["a"].count, 0);
    }

    #[tokio::test]
    async fn disabled_policy_preserves_success_bonus_and_both_response_formats_match() {
        let pool = pool().await;
        sqlx::query("UPDATE accounts SET priority=4 WHERE id='a'")
            .execute(&pool)
            .await
            .unwrap();
        let disabled = Settings::default();
        let mut tracker = CachePriorityTracker::new(&disabled);
        let ticket = tracker.begin("a", &disabled);
        tracker
            .success(&pool, "a", ticket.as_ref(), Some(100), Some(0))
            .await
            .unwrap();
        assert_eq!(priority(&pool).await, 5);
        let settings = Settings {
            low_cache_consecutive: 2,
            ..settings()
        };
        tracker.configure(&settings);
        let json = serde_json::json!({"usage":{"prompt_tokens":100,"prompt_tokens_details":{"cached_tokens":84}}});
        let ordinary = crate::gateway::dto::usage(&json);
        let sse = format!("data: {json}\n\ndata: [DONE]\n\n");
        let streaming = crate::gateway::dto::usage_from_sse(sse.as_bytes());
        assert_eq!(ordinary.0, streaming.0);
        assert_eq!(ordinary.3, streaming.3);
        for tokens in [ordinary, streaming] {
            let ticket = tracker.begin("a", &settings).unwrap();
            tracker
                .success(&pool, "a", Some(&ticket), tokens.0, tokens.3)
                .await
                .unwrap();
        }
        assert_eq!(priority(&pool).await, 2);
    }

    #[tokio::test]
    async fn deduction_stays_above_zero_and_failed_write_does_not_lose_streak() {
        let pool = pool().await;
        let settings = Settings {
            low_cache_consecutive: 1,
            ..settings()
        };
        let mut tracker = CachePriorityTracker::new(&settings);
        let ticket = tracker.begin("a", &settings).unwrap();
        for _ in 0..5 {
            tracker
                .success(&pool, "a", Some(&ticket), Some(100), Some(0))
                .await
                .unwrap();
        }
        assert_eq!(priority(&pool).await, 0);
        let settings = Settings {
            low_cache_consecutive: 2,
            ..settings
        };
        tracker.configure(&settings);
        let ticket = tracker.begin("a", &settings).unwrap();
        tracker
            .success(&pool, "a", Some(&ticket), Some(100), Some(0))
            .await
            .unwrap();
        sqlx::query("CREATE TRIGGER reject_update BEFORE UPDATE ON accounts BEGIN SELECT RAISE(FAIL, 'test'); END")
            .execute(&pool).await.unwrap();
        assert!(tracker
            .success(&pool, "a", Some(&ticket), Some(100), Some(0))
            .await
            .is_err());
        assert_eq!(tracker.accounts["a"].count, 1);
        sqlx::query("DROP TRIGGER reject_update")
            .execute(&pool)
            .await
            .unwrap();
        tracker
            .success(&pool, "a", Some(&ticket), Some(100), Some(0))
            .await
            .unwrap();
        assert_eq!(tracker.accounts["a"].count, 0);
    }
}

struct Counter {
    generation: u64,
    count: u32,
    slow_count: u32,
}

pub struct RequestTicket {
    account_id: String,
    generation: u64,
    cache_epoch: Option<u64>,
    slow_epoch: Option<u64>,
}

pub struct CachePriorityTracker {
    policy: Policy,
    slow_policy: (bool, u32, u64, u8),
    cache_epoch: u64,
    slow_epoch: u64,
    generation: u64,
    accounts: HashMap<String, Counter>,
}

pub type SharedCachePriority = std::sync::Arc<Mutex<CachePriorityTracker>>;

impl CachePriorityTracker {
    fn slow_policy(settings: &Settings) -> (bool, u32, u64, u8) {
        (
            settings.slow_first_token_enabled,
            settings.slow_first_token_consecutive,
            settings.slow_first_token_threshold_seconds,
            settings.slow_first_token_priority_penalty,
        )
    }
    pub fn new(settings: &Settings) -> Self {
        Self {
            policy: Policy::from_settings(settings),
            slow_policy: Self::slow_policy(settings),
            cache_epoch: 0,
            slow_epoch: 0,
            generation: 0,
            accounts: HashMap::new(),
        }
    }

    pub fn configure(&mut self, settings: &Settings) {
        let policy = Policy::from_settings(settings);
        if self.policy != policy {
            self.policy = policy;
            self.cache_epoch += 1;
            for counter in self.accounts.values_mut() {
                counter.count = 0;
            }
        }
        let slow_policy = Self::slow_policy(settings);
        if self.slow_policy != slow_policy {
            self.slow_policy = slow_policy;
            self.slow_epoch += 1;
            for counter in self.accounts.values_mut() {
                counter.slow_count = 0;
            }
        }
        if !self.policy.0 && !self.slow_policy.0 {
            self.accounts.clear();
        }
    }

    pub fn remove(&mut self, account_id: &str) {
        self.accounts.remove(account_id);
    }

    pub fn begin(&mut self, account_id: &str, settings: &Settings) -> Option<RequestTicket> {
        // A request started under old settings must never restore the old policy.
        let cache_epoch = (self.policy == Policy::from_settings(settings) && self.policy.0)
            .then_some(self.cache_epoch);
        let slow_epoch = (self.slow_policy == Self::slow_policy(settings) && self.slow_policy.0)
            .then_some(self.slow_epoch);
        if cache_epoch.is_none() && slow_epoch.is_none() {
            return None;
        }
        let counter = self
            .accounts
            .entry(account_id.to_owned())
            .or_insert_with(|| {
                self.generation += 1;
                Counter {
                    generation: self.generation,
                    count: 0,
                    slow_count: 0,
                }
            });
        Some(RequestTicket {
            account_id: account_id.to_owned(),
            generation: counter.generation,
            cache_epoch,
            slow_epoch,
        })
    }

    fn counter(&mut self, ticket: &RequestTicket) -> Option<&mut Counter> {
        self.accounts
            .get_mut(&ticket.account_id)
            .filter(|counter| counter.generation == ticket.generation)
    }

    pub fn reset(&mut self, ticket: Option<&RequestTicket>) {
        let cache_epoch = self.cache_epoch;
        let slow_epoch = self.slow_epoch;
        if let Some(ticket) = ticket {
            if let Some(counter) = self.counter(ticket) {
                if ticket.cache_epoch == Some(cache_epoch) {
                    counter.count = 0;
                }
                if ticket.slow_epoch == Some(slow_epoch) {
                    counter.slow_count = 0;
                }
            }
        }
    }

    // Called once after a successful gateway response is fully read. Keep the lock
    // through the priority write so concurrent completions cannot reorder mutations.
    pub async fn success(
        &mut self,
        pool: &SqlitePool,
        account_id: &str,
        ticket: Option<&RequestTicket>,
        input: Option<i64>,
        cached: Option<i64>,
    ) -> Result<(), AppError> {
        self.finish_success(pool, account_id, ticket, input, cached, None)
            .await
    }

    pub async fn finish_success(
        &mut self,
        pool: &SqlitePool,
        account_id: &str,
        ticket: Option<&RequestTicket>,
        input: Option<i64>,
        cached: Option<i64>,
        first_token_ms: Option<i64>,
    ) -> Result<(), AppError> {
        let policy = self.policy;
        let slow_policy = self.slow_policy;
        let cache_epoch = self.cache_epoch;
        let slow_epoch = self.slow_epoch;
        let Some(ticket) = ticket else {
            return account_dao::adjust_by(pool, account_id, 1, None, None).await;
        };
        let Some(counter) = self.counter(ticket) else {
            return account_dao::adjust_by(pool, account_id, 1, None, None).await;
        };
        let cache_active = ticket.cache_epoch == Some(cache_epoch);
        let slow_active = ticket.slow_epoch == Some(slow_epoch);
        let low_cache = cache_active
            && matches!((input, cached), (Some(input), Some(cached))
            if input > 0 && cached >= 0 && cached <= input
                && i128::from(cached) * 100 < i128::from(input) * i128::from(policy.2));
        let slow = slow_active
            && first_token_ms
                .is_some_and(|ms| ms >= 0 && i128::from(ms) > i128::from(slow_policy.2) * 1000);
        let next_cache = if low_cache { counter.count + 1 } else { 0 };
        let next_slow = if slow { counter.slow_count + 1 } else { 0 };
        let cache_trigger = low_cache && next_cache >= policy.1;
        let slow_trigger = slow && next_slow >= slow_policy.1;
        let penalty = if cache_trigger {
            i64::from(policy.3)
        } else {
            0
        } + if slow_trigger {
            i64::from(slow_policy.3)
        } else {
            0
        };
        let delta = if penalty > 0 {
            -penalty
        } else if low_cache || slow {
            0
        } else {
            1
        };
        // Commit counters only after the single combined write succeeds.
        account_dao::adjust_by(pool, account_id, delta, None, None).await?;
        if cache_active {
            counter.count = if cache_trigger { 0 } else { next_cache };
        }
        if slow_active {
            counter.slow_count = if slow_trigger { 0 } else { next_slow };
        }
        if penalty > 0 {
            tracing::info!(
                account_id,
                cache_trigger,
                slow_trigger,
                penalty,
                "网关连续表现不佳，扣减账号优先级"
            );
        }
        Ok(())
    }
}
