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
}

pub struct RequestTicket {
    account_id: String,
    generation: u64,
}

pub struct CachePriorityTracker {
    policy: Policy,
    generation: u64,
    accounts: HashMap<String, Counter>,
}

pub type SharedCachePriority = std::sync::Arc<Mutex<CachePriorityTracker>>;

impl CachePriorityTracker {
    pub fn new(settings: &Settings) -> Self {
        Self {
            policy: Policy::from_settings(settings),
            generation: 0,
            accounts: HashMap::new(),
        }
    }

    pub fn configure(&mut self, settings: &Settings) {
        let policy = Policy::from_settings(settings);
        if self.policy != policy {
            self.policy = policy;
            self.accounts.clear();
        }
    }

    pub fn remove(&mut self, account_id: &str) {
        self.accounts.remove(account_id);
    }

    pub fn begin(&mut self, account_id: &str, settings: &Settings) -> Option<RequestTicket> {
        // A request started under old settings must never restore the old policy.
        if self.policy != Policy::from_settings(settings) || !self.policy.0 {
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
                }
            });
        Some(RequestTicket {
            account_id: account_id.to_owned(),
            generation: counter.generation,
        })
    }

    fn counter(&mut self, ticket: &RequestTicket) -> Option<&mut Counter> {
        self.accounts
            .get_mut(&ticket.account_id)
            .filter(|counter| counter.generation == ticket.generation)
    }

    pub fn reset(&mut self, ticket: Option<&RequestTicket>) {
        if let Some(counter) = ticket.and_then(|ticket| self.counter(ticket)) {
            counter.count = 0;
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
        let policy = self.policy;
        let counter = ticket.and_then(|ticket| self.counter(ticket));
        let delta = match (counter, input, cached) {
            (Some(counter), Some(input), Some(cached))
                if input > 0 && cached >= 0 && cached <= input =>
            {
                if i128::from(cached) * 100 < i128::from(input) * i128::from(policy.2) {
                    let next = counter.count + 1;
                    if next >= policy.1 {
                        // Reset only after persistence succeeds, allowing the next
                        // low-cache completion to retry a failed deduction.
                        account_dao::adjust_by(pool, account_id, -i64::from(policy.3), None, None)
                            .await?;
                        counter.count = 0;
                        tracing::info!(
                            account_id,
                            consecutive = policy.1,
                            threshold_percent = policy.2,
                            penalty = policy.3,
                            "连续低缓存率，扣减账号优先级"
                        );
                        return Ok(());
                    }
                    counter.count = next;
                    0
                } else {
                    counter.count = 0;
                    1
                }
            }
            (Some(counter), _, _) => {
                counter.count = 0;
                1
            }
            _ => 1,
        };
        account_dao::adjust_by(pool, account_id, delta, None, None).await
    }
}
