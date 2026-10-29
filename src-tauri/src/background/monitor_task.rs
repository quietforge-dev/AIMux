use std::time::Duration;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use futures_util::{stream, StreamExt};
use tokio::time::{interval, MissedTickBehavior};

use crate::{
    app_state::AppState,
    dao::{account_dao, monitor_dao},
    model::{account::Account, monitor_record::MonitorRecord},
    service::{account_probe_service, account_service},
    upstream::{error::response_body, first_token::FirstTokenDeadline},
    utils::time::{elapsed_millis, utc_now_string},
};

const MONITOR_CONCURRENCY: usize = 5;

pub async fn run(state: Arc<AppState>) {
    let mut ticker = interval(Duration::from_secs(5));
    let mut last_started: Option<tokio::time::Instant> = None;
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        let settings = state.settings.read().await.clone();
        if !settings.monitoring_enabled
            || !crate::utils::time::in_monitoring_window(
                settings.monitoring_start_time.as_deref(),
                settings.monitoring_end_time.as_deref(),
                chrono::Local::now().time(),
            )
        {
            continue;
        }
        if !monitoring_due(
            last_started.map(|last| last.elapsed()),
            settings.monitoring_interval_minutes,
        ) {
            continue;
        }
        last_started = Some(tokio::time::Instant::now());
        if let Err(error) = round(&state).await {
            tracing::error!(%error, "监控轮次失败");
        }
    }
}

fn monitoring_due(elapsed: Option<Duration>, interval_minutes: u64) -> bool {
    let interval = Duration::from_secs(interval_minutes.clamp(2, 10) * 60);
    elapsed.is_none_or(|elapsed| elapsed >= interval)
}

#[cfg(test)]
mod interval_tests {
    use super::monitoring_due;
    use std::time::Duration;

    #[test]
    fn first_round_and_exact_boundary_are_due() {
        assert!(monitoring_due(None, 2));
        assert!(!monitoring_due(Some(Duration::from_secs(119)), 2));
        assert!(monitoring_due(Some(Duration::from_secs(120)), 2));
    }

    #[test]
    fn changed_interval_uses_time_since_last_round() {
        let elapsed = Some(Duration::from_secs(180));
        assert!(monitoring_due(elapsed, 2));
        assert!(!monitoring_due(elapsed, 10));
        assert!(monitoring_due(Some(Duration::from_secs(600)), 10));
    }
}

async fn round(state: &Arc<AppState>) -> Result<(), crate::error::AppError> {
    let prompt = random_probe_prompt();
    let (accounts, _) =
        account_dao::list(&state.pool, 0, 10_000, None, Some("active"), None).await?;
    let mut completed = Vec::with_capacity(accounts.len());
    let mut pending = stream::iter(accounts.into_iter().enumerate())
        .map(|(index, account)| async move {
            monitor_account(state, account, prompt)
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
    prompt: &'static str,
) -> Result<Option<(String, String, bool, String)>, crate::error::AppError> {
    let Some(model) = account
        .test_default_model
        .clone()
        .or(crate::dao::model_dao::default_name(&state.pool, &account.r#type).await?)
    else {
        return Ok(None);
    };
    let probe = account_probe_service::build_stream_request(&account, &model, prompt);
    let model = probe.model.clone();
    let started = std::time::Instant::now();
    let settings = state.settings.read().await.clone();
    let first_token_timeout = Duration::from_secs(settings.first_token_timeout_seconds.max(1));
    let first_token_deadline = FirstTokenDeadline::from_start(started, first_token_timeout);
    let result = match first_token_deadline
        .run(account_probe_service::send(
            &account,
            &probe,
            &settings,
            Some(first_token_timeout),
        ))
        .await
    {
        Err(_) => (
            false,
            Some(504),
            None,
            Some("等待监控流式首字超时".to_owned()),
        ),
        Ok(Err(error)) => {
            tracing::error!(account_id = %account.id, account_name = %account.name, %error, "账号监控连接失败");
            let status = if matches!(error, crate::error::AppError::UpstreamTimeout(_)) {
                504
            } else {
                502
            };
            (false, Some(status), None, Some(error.to_string()))
        }
        Ok(Ok(mut response)) => {
            let status = response.status();
            let (success, first_token_ms, message) = if status.is_success() {
                match wait_for_first_token(&mut response, &first_token_deadline).await {
                    Ok(()) => (true, Some(elapsed_millis(started)), None),
                    Err(message) => (false, None, Some(message)),
                }
            } else {
                let bytes = response.bytes().await.unwrap_or_default();
                let response_body = response_body(&bytes);
                (false, None, Some(response_body))
            };
            if success {
                tracing::info!(
                    account_id = %account.id,
                    account_name = %account.name,
                    status_code = status.as_u16(),
                    "账号监控成功"
                );
            } else {
                tracing::error!(
                    account_id = %account.id,
                    account_name = %account.name,
                    status_code = status.as_u16(),
                    "账号监控失败"
                );
            }
            (
                success,
                Some(status.as_u16() as i64),
                first_token_ms,
                message,
            )
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
        result.3.as_deref(),
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
            duration_ms: Some(result.2.unwrap_or_else(|| elapsed_millis(started))),
            success: result.0,
            status_code: result.1,
            error_code: if result.0 {
                None
            } else {
                Some("monitor_failed".into())
            },
            error_message: result.3,
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

const PROBE_PROMPTS: [&str; 10] = [
    "ping. Reply with pong only.",
    "hello. Reply with one word.",
    "What model are you? Answer briefly.",
    "Reply with one short greeting.",
    "What is 1 + 1? Reply with one character.",
    "Complete this: the sky is",
    "Reply with exactly one word: ready",
    "Name one color.",
    "Say yes or no: is water wet?",
    "Write a one-word thank you.",
];

fn random_probe_prompt() -> &'static str {
    let bytes = uuid::Uuid::new_v4().into_bytes();
    PROBE_PROMPTS[usize::from(bytes[0]) % PROBE_PROMPTS.len()]
}

async fn wait_for_first_token(
    response: &mut reqwest::Response,
    deadline: &FirstTokenDeadline,
) -> Result<(), String> {
    let is_sse = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().contains("text/event-stream"));
    let mut pending = Vec::new();
    let mut received = 0;
    loop {
        let chunk = deadline
            .run(response.chunk())
            .await
            .map_err(|_| "等待监控流式首字超时".to_owned())?
            .map_err(|error| format!("读取监控流失败：{error}"))?;
        let Some(chunk) = chunk else {
            if let Ok(event) = serde_json::from_slice::<serde_json::Value>(&pending) {
                if let Some(result) = probe_event_result(&event) {
                    return result;
                }
            }
            // Some relays omit the final newline from the last SSE event.
            let tail = String::from_utf8_lossy(&pending);
            if let Some(data) = tail.trim().strip_prefix("data:") {
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(data.trim()) {
                    if let Some(result) = probe_event_result(&event) {
                        return result;
                    }
                }
            }
            return Err("监控响应结束，但未识别到文本内容".into());
        };
        received += chunk.len();
        pending.extend_from_slice(&chunk);
        if received > 256 * 1024 {
            return Err("监控流数据超过限制且未返回首字".into());
        }
        if !is_sse
            && pending
                .iter()
                .copied()
                .find(|byte| !byte.is_ascii_whitespace())
                == Some(b'{')
        {
            if let Ok(event) = serde_json::from_slice::<serde_json::Value>(&pending) {
                return probe_event_result(&event)
                    .unwrap_or_else(|| Err("上游返回 JSON，但未包含可识别的文本内容".into()));
            }
            continue;
        }
        while let Some(newline) = pending.iter().position(|byte| *byte == b'\n') {
            let line = String::from_utf8_lossy(&pending[..newline])
                .trim()
                .to_owned();
            pending.drain(..=newline);
            let Some(data) = line.strip_prefix("data:").map(str::trim) else {
                continue;
            };
            if data == "[DONE]" {
                return Err("监控流在返回首字前结束".into());
            }
            let Ok(event) = serde_json::from_str::<serde_json::Value>(data) else {
                continue;
            };
            if let Some(result) = probe_event_result(&event) {
                return result;
            }
        }
    }
}

fn probe_event_result(event: &serde_json::Value) -> Option<Result<(), String>> {
    let bytes = serde_json::to_vec(event).unwrap_or_default();
    if crate::gateway::dto::stream_outcome(&bytes) == Some(false) {
        return Some(Err(crate::gateway::dto::stream_failure_message(&bytes)));
    }
    if stream_event_has_text(event) {
        return Some(Ok(()));
    }
    let finish = event
        .pointer("/choices/0/finish_reason")
        .and_then(serde_json::Value::as_str);
    if finish == Some("length") {
        return Some(Err(
            "监控生成额度耗尽但没有可见文本（可能仅生成了推理内容）".into(),
        ));
    }
    if finish.is_some() || crate::gateway::dto::stream_outcome(&bytes) == Some(true) {
        return Some(Err("监控响应正常结束，但未返回可识别的文本内容".into()));
    }
    None
}

fn stream_event_has_text(event: &serde_json::Value) -> bool {
    let openai_chat = event
        .pointer("/choices/0/delta/content")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|text| !text.is_empty());
    let openai_responses = event.get("type").and_then(serde_json::Value::as_str)
        == Some("response.output_text.delta")
        && event
            .get("delta")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|text| !text.is_empty());
    let anthropic = event
        .pointer("/delta/type")
        .and_then(serde_json::Value::as_str)
        == Some("text_delta")
        && event
            .pointer("/delta/text")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|text| !text.is_empty());
    let initial_text = event
        .pointer("/content_block/text")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|text| !text.is_empty());
    let json_chat = event
        .pointer("/choices/0/message/content")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|text| !text.is_empty());
    let text_parts = event
        .get("content")
        .or_else(|| event.pointer("/message/content"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|parts| {
            parts.iter().any(|part| {
                part.get("text")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|text| !text.is_empty())
            })
        });
    let completion_text = event
        .pointer("/choices/0/text")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|text| !text.is_empty());
    openai_chat
        || openai_responses
        || anthropic
        || initial_text
        || json_chat
        || text_parts
        || completion_text
}

#[cfg(test)]
mod probe_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn differentiates_empty_budget_exhaustion_upstream_error_and_json_content() {
        assert!(probe_event_result(
            &json!({"choices":[{"message":{"content":"OK"},"finish_reason":"length"}]})
        )
        .unwrap()
        .is_ok());
        assert!(
            probe_event_result(&json!({"content":[{"type":"text","text":"OK"}]}))
                .unwrap()
                .is_ok()
        );
        let error = probe_event_result(&json!({"choices":[{"delta":{},"finish_reason":"length"}]}))
            .unwrap()
            .unwrap_err();
        assert!(error.contains("额度耗尽"));
        let error = probe_event_result(&json!({"type":"response.incomplete","response":{"incomplete_details":{"reason":"max_output_tokens"}}})).unwrap().unwrap_err();
        assert!(error.contains("max_output_tokens"));
        assert!(probe_event_result(
            &json!({"error":null,"choices":[{"delta":{"role":"assistant"}}]})
        )
        .is_none());
    }

    #[test]
    fn ignores_keepalive_role_and_reasoning_events() {
        for event in [
            json!({"choices": [{"delta": {"role": "assistant", "content": ""}}]}),
            json!({"choices": [{"delta": {"reasoning_content": "thinking"}}]}),
            json!({"type": "response.reasoning_summary_text.delta", "delta": "thinking"}),
            json!({"type": "content_block_delta", "delta": {"type": "thinking_delta", "thinking": "thinking"}}),
        ] {
            assert!(!stream_event_has_text(&event));
        }
        for event in [
            json!({"choices": [{"delta": {"content": "hello"}}]}),
            json!({"type": "response.output_text.delta", "delta": "hello"}),
            json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "你好"}}),
        ] {
            assert!(stream_event_has_text(&event));
        }
    }

    async fn probe_response(parts: Vec<&'static str>) -> reqwest::Response {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = axum::Router::new().route(
            "/",
            axum::routing::get(move || async move {
                let body = axum::body::Body::from_stream(futures_util::stream::iter(
                    parts.into_iter().map(|part| Ok::<_, std::io::Error>(part)),
                ));
                axum::response::Response::builder()
                    .header("content-type", "text/event-stream")
                    .body(body)
                    .unwrap()
            }),
        );
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let response = reqwest::get(format!("http://{address}/")).await.unwrap();
        task.abort();
        response
    }

    #[tokio::test]
    async fn reads_fragmented_sse_and_rejects_empty_and_error_streams() {
        for body in [
            "data: {\"choices\":[{\"delta\":{\"content\":\"OK\"}}]}",
            "{\"choices\":[{\"message\":{\"content\":\"OK\"}}]}",
        ] {
            let mut response = probe_response(vec![body]).await;
            assert!(wait_for_first_token(
                &mut response,
                &FirstTokenDeadline::from_start(std::time::Instant::now(), Duration::from_secs(1),),
            )
            .await
            .is_ok());
        }
        let mut response = probe_response(vec![
            ": keepalive\n\ndata: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"con",
            "tent\":\"hello\"}}]}\n\n",
        ])
        .await;
        assert!(wait_for_first_token(
            &mut response,
            &FirstTokenDeadline::from_start(std::time::Instant::now(), Duration::from_secs(1),),
        )
        .await
        .is_ok());
        for parts in [
            vec!["data: [DONE]\n\n"],
            vec!["data: {\"error\":{\"message\":\"failed\"}}\n\n"],
            vec![": heartbeat\n\n"],
        ] {
            let mut response = probe_response(parts).await;
            assert!(wait_for_first_token(
                &mut response,
                &FirstTokenDeadline::from_start(std::time::Instant::now(), Duration::from_secs(1),),
            )
            .await
            .is_err());
        }
    }
}

async fn rebalance(
    pool: &sqlx::SqlitePool,
    results: Vec<(String, String, bool, String)>,
) -> Result<(), crate::error::AppError> {
    for ((account_type, model), successful_ids) in successful_accounts_by_probe_model(&results) {
        let (accounts, _) =
            account_dao::list(pool, 0, 10_000, Some(&account_type), Some("active"), None).await?;
        let Some(candidate) = accounts
            .iter()
            .filter(|account| successful_ids.contains(&account.id))
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
        let current = account_dao::pick_one(pool, Some(&model), &account_type).await?;
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

pub fn successful_accounts_by_probe_model(
    results: &[(String, String, bool, String)],
) -> BTreeMap<(String, String), BTreeSet<String>> {
    let mut grouped = BTreeMap::new();
    for (account_id, account_type, success, model) in results {
        if *success {
            grouped
                .entry((account_type.clone(), model.clone()))
                .or_insert_with(BTreeSet::new)
                .insert(account_id.clone());
        }
    }
    grouped
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
