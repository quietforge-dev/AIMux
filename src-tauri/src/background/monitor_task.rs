use std::time::Duration;
use std::{cmp::Ordering, sync::Arc};

use futures_util::{stream, StreamExt};
use tokio::time::{interval, MissedTickBehavior};

use crate::{
    app_state::AppState,
    dao::{account_dao, model_dao, monitor_dao},
    model::{account::Account, monitor_record::MonitorRecord},
    service::account_service,
    utils::time::{elapsed_millis, utc_now_string},
};

const MONITOR_CONCURRENCY: usize = 5;
const MONITOR_REQUEST_TIMEOUT: Duration = Duration::from_secs(40);

pub async fn run(state: Arc<AppState>) {
    let mut ticker = interval(Duration::from_secs(120));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        if !state.settings.read().await.monitoring_enabled {
            continue;
        }
        if let Err(error) = round(&state).await {
            tracing::error!(%error, "监控轮次失败");
        }
    }
}

async fn round(state: &Arc<AppState>) -> Result<(), crate::error::AppError> {
    let (accounts, _) = account_dao::list(&state.pool, 0, 10_000, None, Some("active")).await?;
    let mut completed = Vec::with_capacity(accounts.len());
    let mut pending = stream::iter(accounts.into_iter().enumerate())
        .map(|(index, account)| async move {
            monitor_account(state, account)
                .await
                .map(|result| result.map(|result| (index, result)))
        })
        .buffer_unordered(MONITOR_CONCURRENCY);
    while let Some(result) = pending.next().await {
        if let Some(result) = result? {
            completed.push(result);
        }
    }
    completed.sort_unstable_by_key(|(index, _)| *index);
    let results = completed.into_iter().map(|(_, result)| result).collect();
    rebalance(&state.pool, results).await?;
    Ok(())
}

async fn monitor_account(
    state: &AppState,
    account: Account,
) -> Result<Option<(String, String, bool, String)>, crate::error::AppError> {
    let model = match account.test_default_model.clone() {
        Some(value) => Some(value),
        None => model_dao::default_name(&state.pool, &account.r#type).await?,
    };
    let Some(model) = model else {
        return Ok(None);
    };
    let started = std::time::Instant::now();
    let settings = state.settings.read().await.clone();
    let endpoint = if account.r#type == "anthropic" {
        "/v1/messages"
    } else {
        "/v1/chat/completions"
    };
    let upstream_model = account_service::mapping(&account, Some(&model)).unwrap_or(model.clone());
    let body = if account.r#type == "anthropic" {
        serde_json::json!({"model": upstream_model, "max_tokens": 1, "messages": [{"role":"user","content":"ping"}]})
    } else {
        serde_json::json!({"model": upstream_model, "max_tokens": 1, "reasoning_effort":"low", "messages": [{"role":"user","content":"ping"}]})
    };
    let result = match crate::upstream::client::post_with_timeout(
        &account,
        endpoint,
        &body,
        &settings,
        &[],
        MONITOR_REQUEST_TIMEOUT,
    )
    .await
    {
        Ok(response) => {
            let status = response.status();
            let success = status.is_success();
            let message = if success {
                tracing::info!(
                    account_id = %account.id,
                    account_name = %account.name,
                    status_code = status.as_u16(),
                    "账号监控成功"
                );
                None
            } else {
                let bytes = response.bytes().await.unwrap_or_default();
                let response_body =
                    String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]).to_string();
                tracing::error!(
                    account_id = %account.id,
                    account_name = %account.name,
                    status_code = status.as_u16(),
                    "账号监控失败"
                );
                Some(response_body)
            };
            (success, Some(status.as_u16() as i64), message)
        }
        Err(error) => {
            tracing::error!(account_id = %account.id, account_name = %account.name, %error, "账号监控连接失败");
            (false, Some(502), Some(error.to_string()))
        }
    };
    account_service::record_monitor(
        &state.pool,
        &account.id,
        result.0,
        if result.0 {
            None
        } else {
            Some("monitor_failed")
        },
        result.2.as_deref(),
    )
    .await?;
    let result_account_id = account.id.clone();
    let result_account_type = account.r#type.clone();
    let result_model = model.clone();
    monitor_dao::create_and_refresh_account_average(
        &state.pool,
        &MonitorRecord {
            id: uuid::Uuid::new_v4().to_string(),
            account_id: result_account_id.clone(),
            account_name: account.name.clone(),
            account_type: result_account_type.clone(),
            model: Some(result_model.clone()),
            checked_at: utc_now_string(),
            duration_ms: Some(elapsed_millis(started)),
            success: result.0,
            status_code: result.1,
            error_code: if result.0 {
                None
            } else {
                Some("monitor_failed".into())
            },
            error_message: result.2,
        },
    )
    .await?;
    Ok(Some((
        result_account_id,
        result_account_type,
        result.0,
        result_model,
    )))
}

async fn rebalance(
    pool: &sqlx::SqlitePool,
    results: Vec<(String, String, bool, String)>,
) -> Result<(), crate::error::AppError> {
    for kind in ["openai", "anthropic"] {
        let successful: Vec<_> = results
            .iter()
            .filter(|(_, account_type, success, _)| account_type == kind && *success)
            .collect();
        let Some((_, _, _, model)) = successful.first() else {
            continue;
        };
        let (accounts, _) = account_dao::list(pool, 0, 10_000, Some(kind), Some("active")).await?;
        let Some(candidate) = accounts
            .iter()
            .filter(|account| successful.iter().any(|(id, _, _, _)| id == &account.id))
            .min_by(|left, right| {
                candidate_order(
                    left.multiplier,
                    left.monitor_average_duration_ms,
                    right.multiplier,
                    right.monitor_average_duration_ms,
                )
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
                .then_with(|| left.id.cmp(&right.id))
            })
        else {
            continue;
        };
        let current = account_dao::pick_one(pool, Some(model), kind).await?;
        if let Some(current) = current {
            if current.id != candidate.id
                && should_promote(
                    candidate.multiplier,
                    candidate.monitor_average_duration_ms,
                    current.multiplier,
                    current.monitor_average_duration_ms,
                )
            {
                account_dao::save_priority(pool, &candidate.id, 9).await?;
            }
        }
    }
    Ok(())
}

pub fn candidate_order(
    left_multiplier: f64,
    left_average_duration_ms: Option<i64>,
    right_multiplier: f64,
    right_average_duration_ms: Option<i64>,
) -> Ordering {
    left_multiplier.total_cmp(&right_multiplier).then_with(|| {
        left_average_duration_ms
            .unwrap_or(i64::MAX)
            .cmp(&right_average_duration_ms.unwrap_or(i64::MAX))
    })
}

pub fn should_promote(
    candidate_multiplier: f64,
    candidate_average_duration_ms: Option<i64>,
    current_multiplier: f64,
    current_average_duration_ms: Option<i64>,
) -> bool {
    match candidate_multiplier.total_cmp(&current_multiplier) {
        Ordering::Less => true,
        Ordering::Greater => false,
        Ordering::Equal => {
            candidate_average_duration_ms.unwrap_or(i64::MAX)
                < current_average_duration_ms.unwrap_or(i64::MAX)
        }
    }
}
