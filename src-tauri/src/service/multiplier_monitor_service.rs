use std::{collections::HashMap, str::FromStr, time::Duration};

use chrono::{Duration as ChronoDuration, Utc};
use futures_util::{stream, StreamExt};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    config::Settings,
    dao::multiplier_monitor_dao::{self, AccountMultiplierChange},
    error::AppError,
    model::multiplier_monitor::{MultiplierMonitorConfig, MultiplierMonitorLog},
    schema::multiplier_monitor_schema::{
        MultiplierMonitorCheckResult, MultiplierMonitorConfigView, MultiplierMonitorCreate,
        MultiplierMonitorRunSummary, MultiplierMonitorUpdate,
    },
    service::multiplier_query::{self, MultiplierQueryError, RemoteMultiplier},
    utils::time::{elapsed_millis, utc_datetime_string, utc_now_string},
};

const MAX_ACCOUNT_COUNT: usize = 10_000;
const QUERY_TOTAL_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const LEASE_DURATION_MINUTES: i64 = 10;
const SCHEDULE_INTERVAL_HOURS: i64 = 1;
const MAX_ERROR_LENGTH: usize = 500;
const CONFIG_CONCURRENCY: usize = 2;
const MAX_MULTIPLIER_DIVISOR: i64 = 10_000;

#[derive(Clone)]
struct ParsedRemoteRate {
    decimal: Option<Decimal>,
    value: Option<f64>,
    error: Option<String>,
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<MultiplierMonitorConfigView>, AppError> {
    let configs = multiplier_monitor_dao::list_configs(pool).await?;
    let now = utc_now_string();
    let mut views = Vec::with_capacity(configs.len());
    for config in configs {
        views.push(to_view(pool, config, &now).await?);
    }
    Ok(views)
}

pub async fn get_view(
    pool: &SqlitePool,
    id: &str,
) -> Result<MultiplierMonitorConfigView, AppError> {
    let config = multiplier_monitor_dao::get_config(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("倍率监控配置不存在".into()))?;
    to_view(pool, config, &utc_now_string()).await
}

pub async fn create(
    pool: &SqlitePool,
    payload: MultiplierMonitorCreate,
) -> Result<MultiplierMonitorConfigView, AppError> {
    let name = validate_name(&payload.name)?;
    let url = validate_url(&payload.url)?;
    let token = normalize_token(&payload.token)?;
    let refresh_token = normalize_refresh_token(&payload.refresh_token)?;
    let multiplier_divisor = validate_multiplier_divisor(payload.multiplier_divisor)?;
    let account_ids = normalize_account_ids(payload.account_ids)?;
    validate_accounts(pool, &account_ids).await?;
    validate_enabled_conflicts(pool, None, payload.enabled, &account_ids).await?;
    let account_ids_json = serde_json::to_string(&account_ids)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let config = multiplier_monitor_dao::create_config(
        pool,
        &name,
        &url,
        &token,
        &refresh_token,
        &account_ids_json,
        multiplier_divisor,
        payload.enabled,
        &utc_now_string(),
    )
    .await?;
    to_view(pool, config, &utc_now_string()).await
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    payload: MultiplierMonitorUpdate,
) -> Result<MultiplierMonitorConfigView, AppError> {
    let current = multiplier_monitor_dao::get_config(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("倍率监控配置不存在".into()))?;
    let name = validate_name(&payload.name)?;
    let url = validate_url(&payload.url)?;
    let token = match payload.token.as_deref().map(str::trim) {
        None | Some("") => current.token.clone(),
        Some(value) => normalize_token(value)?,
    };
    let refresh_token = match payload.refresh_token.as_deref().map(str::trim) {
        None | Some("") => current.refresh_token.clone(),
        Some(value) => normalize_refresh_token(value)?,
    };
    let multiplier_divisor = validate_multiplier_divisor(
        payload
            .multiplier_divisor
            .unwrap_or(current.multiplier_divisor),
    )?;
    let account_ids = normalize_account_ids(payload.account_ids)?;
    validate_accounts(pool, &account_ids).await?;
    validate_enabled_conflicts(pool, Some(id), payload.enabled, &account_ids).await?;
    let account_ids_json = serde_json::to_string(&account_ids)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let reset_schedule = current.url != url
        || current.token != token
        || current.refresh_token != refresh_token
        || current.account_ids != account_ids_json
        || current.multiplier_divisor != multiplier_divisor;
    let config = multiplier_monitor_dao::update_config(
        pool,
        id,
        &name,
        &url,
        &token,
        &refresh_token,
        &account_ids_json,
        multiplier_divisor,
        payload.enabled,
        reset_schedule,
        &utc_now_string(),
    )
    .await?
    .ok_or_else(|| AppError::NotFound("倍率监控配置不存在".into()))?;
    to_view(pool, config, &utc_now_string()).await
}

pub async fn toggle(pool: &SqlitePool, id: &str) -> Result<MultiplierMonitorConfigView, AppError> {
    let current = multiplier_monitor_dao::get_config(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("倍率监控配置不存在".into()))?;
    if !current.enabled {
        let account_ids = parse_account_ids(&current.account_ids)?;
        validate_accounts(pool, &account_ids).await?;
        validate_enabled_conflicts(pool, Some(id), true, &account_ids).await?;
    }
    let config = multiplier_monitor_dao::toggle_config(pool, id, &utc_now_string())
        .await?
        .ok_or_else(|| AppError::NotFound("倍率监控配置不存在".into()))?;
    to_view(pool, config, &utc_now_string()).await
}

pub async fn check_now(
    pool: &SqlitePool,
    settings: &Settings,
    id: &str,
) -> Result<MultiplierMonitorCheckResult, AppError> {
    let owner = format!("manual-{}", Uuid::new_v4());
    let config = claim(pool, id, &owner, true)
        .await?
        .ok_or_else(|| AppError::BadRequest("配置已停用或正在检查中".into()))?;
    run_claimed(pool, settings, config, &owner).await
}

pub async fn run_due(
    pool: &SqlitePool,
    settings: &Settings,
    worker_id: &str,
) -> Result<(), AppError> {
    let now = Utc::now();
    let due_before = utc_datetime_string(now - ChronoDuration::hours(SCHEDULE_INTERVAL_HOURS));
    let now_text = utc_datetime_string(now);
    let ids = multiplier_monitor_dao::list_due_ids(pool, &due_before, &now_text).await?;
    let mut pending = stream::iter(ids)
        .map(|id| async move {
            let owner = format!("{worker_id}-{}", Uuid::new_v4());
            let Some(config) = claim(pool, &id, &owner, false).await? else {
                return Ok::<_, AppError>(());
            };
            if let Err(error) = run_claimed(pool, settings, config, &owner).await {
                tracing::error!(config_id = %id, %error, "倍率监控配置执行失败");
            }
            Ok(())
        })
        .buffer_unordered(CONFIG_CONCURRENCY);
    while let Some(result) = pending.next().await {
        if let Err(error) = result {
            tracing::error!(%error, "倍率监控配置领取失败");
        }
    }
    Ok(())
}

async fn claim(
    pool: &SqlitePool,
    id: &str,
    owner: &str,
    force: bool,
) -> Result<Option<MultiplierMonitorConfig>, AppError> {
    let now = Utc::now();
    multiplier_monitor_dao::claim_config(
        pool,
        id,
        owner,
        &utc_datetime_string(now),
        &utc_datetime_string(now + ChronoDuration::minutes(LEASE_DURATION_MINUTES)),
        &utc_datetime_string(now - ChronoDuration::hours(SCHEDULE_INTERVAL_HOURS)),
        force,
    )
    .await
}

async fn run_claimed(
    pool: &SqlitePool,
    settings: &Settings,
    config: MultiplierMonitorConfig,
    owner: &str,
) -> Result<MultiplierMonitorCheckResult, AppError> {
    let started_at = config
        .last_started_at
        .clone()
        .unwrap_or_else(utc_now_string);
    let started = std::time::Instant::now();
    let mut config = config;
    let query = tokio::time::timeout(
        QUERY_TOTAL_TIMEOUT,
        fetch_with_refresh(pool, settings, &mut config, owner),
    )
    .await;
    let remote = match query {
        Ok(Ok(items)) => items,
        Ok(Err(FetchRunError::Query(error))) => {
            return finish_query_failure(pool, &config, owner, &started_at, started, error).await;
        }
        Ok(Err(FetchRunError::App(error))) => {
            let _ =
                multiplier_monitor_dao::release_config(pool, &config.id, owner, &utc_now_string())
                    .await;
            return Err(error);
        }
        Err(_) => {
            return finish_query_failure(
                pool,
                &config,
                owner,
                &started_at,
                started,
                MultiplierQueryError {
                    code: "query_timeout".into(),
                    message: "倍率查询超过整体 5 分钟限制".into(),
                    http_status: None,
                },
            )
            .await;
        }
    };
    let remote_by_key = match remote_map(remote, config.multiplier_divisor) {
        Ok(map) => map,
        Err(error) => {
            return finish_query_failure(pool, &config, owner, &started_at, started, error).await;
        }
    };
    let account_ids = parse_account_ids(&config.account_ids)?;
    let accounts = multiplier_monitor_dao::get_accounts_by_ids(pool, &account_ids).await?;
    let accounts = accounts
        .into_iter()
        .map(|account| (account.id.clone(), account))
        .collect::<HashMap<_, _>>();
    let finished_at = utc_now_string();
    let duration_ms = elapsed_millis(started);
    let run_id = Uuid::new_v4().to_string();
    let mut logs = Vec::with_capacity(account_ids.len());
    let mut changes = Vec::new();

    for account_id in account_ids {
        let Some(account) = accounts.get(&account_id) else {
            logs.push(account_log(
                &run_id,
                &config,
                Some(account_id),
                Some("账号已删除".into()),
                &started_at,
                &finished_at,
                duration_ms,
                "account_missing",
                None,
                None,
                Some("account_missing"),
                Some("配置中的账号已不存在"),
            ));
            continue;
        };
        if account.status != "active" {
            logs.push(account_log(
                &run_id,
                &config,
                Some(account.id.clone()),
                Some(account.name.clone()),
                &started_at,
                &finished_at,
                duration_ms,
                "skipped_disabled",
                Some(account.multiplier),
                None,
                Some("account_disabled"),
                Some("账号已停用，未更新倍率"),
            ));
            continue;
        }
        let account_key = account.api_key_encrypted.trim();
        let Some(remote_rate) = remote_by_key.get(account_key) else {
            logs.push(account_log(
                &run_id,
                &config,
                Some(account.id.clone()),
                Some(account.name.clone()),
                &started_at,
                &finished_at,
                duration_ms,
                "missing_key",
                Some(account.multiplier),
                None,
                Some("key_not_found"),
                Some("远端结果中未找到该账号密钥"),
            ));
            continue;
        };
        if let Some(error) = remote_rate.error.as_deref() {
            logs.push(account_log(
                &run_id,
                &config,
                Some(account.id.clone()),
                Some(account.name.clone()),
                &started_at,
                &finished_at,
                duration_ms,
                "invalid_rate",
                Some(account.multiplier),
                remote_rate.value,
                Some("invalid_rate"),
                Some(error),
            ));
            continue;
        }
        let Some(remote_decimal) = remote_rate.decimal else {
            continue;
        };
        let current_decimal = Decimal::from_str(&format!("{:.2}", account.multiplier))
            .map_err(|error| AppError::Internal(error.to_string()))?;
        if current_decimal == remote_decimal {
            logs.push(account_log(
                &run_id,
                &config,
                Some(account.id.clone()),
                Some(account.name.clone()),
                &started_at,
                &finished_at,
                duration_ms,
                "unchanged",
                Some(account.multiplier),
                remote_rate.value,
                None,
                None,
            ));
            continue;
        }
        let log_index = logs.len();
        logs.push(account_log(
            &run_id,
            &config,
            Some(account.id.clone()),
            Some(account.name.clone()),
            &started_at,
            &finished_at,
            duration_ms,
            "updated",
            Some(account.multiplier),
            remote_rate.value,
            None,
            None,
        ));
        changes.push(AccountMultiplierChange {
            log_index,
            account_id: account.id.clone(),
            expected_key: account.api_key_encrypted.clone(),
            expected_multiplier: account.multiplier,
            next_multiplier: remote_rate.value.unwrap_or(account.multiplier),
        });
    }

    match multiplier_monitor_dao::complete_run(
        pool,
        &config,
        owner,
        &finished_at,
        &mut logs,
        &changes,
    )
    .await
    {
        Ok(true) => Ok(summarize(&run_id, &logs)),
        Ok(false) => {
            let log = account_log(
                &run_id,
                &config,
                None,
                None,
                &started_at,
                &finished_at,
                duration_ms,
                "cancelled",
                None,
                None,
                Some("config_changed"),
                Some("检查期间配置已编辑或停用，本轮未更新"),
            );
            multiplier_monitor_dao::insert_log_and_release(
                pool,
                &log,
                &config.id,
                owner,
                &finished_at,
            )
            .await?;
            Ok(summarize(&run_id, &[log]))
        }
        Err(error) => {
            let _ =
                multiplier_monitor_dao::release_config(pool, &config.id, owner, &finished_at).await;
            Err(error)
        }
    }
}

enum FetchRunError {
    Query(MultiplierQueryError),
    App(AppError),
}

async fn fetch_with_refresh(
    pool: &SqlitePool,
    settings: &Settings,
    config: &mut MultiplierMonitorConfig,
    owner: &str,
) -> Result<Vec<RemoteMultiplier>, FetchRunError> {
    let first = multiplier_query::fetch_all(&config.url, &config.token, settings).await;
    let error = match first {
        Ok(items) => return Ok(items),
        Err(error) => error,
    };
    if error.http_status != Some(401) {
        return Err(FetchRunError::Query(error));
    }
    if config.refresh_token.trim().is_empty() {
        return Err(FetchRunError::Query(MultiplierQueryError {
            code: "unauthorized_no_refresh_token".into(),
            message: "倍率查询接口返回 401，但未配置 refresh token".into(),
            http_status: Some(401),
        }));
    }
    let refreshed = multiplier_query::refresh_tokens(&config.url, &config.refresh_token, settings)
        .await
        .map_err(FetchRunError::Query)?;
    let updated = multiplier_monitor_dao::update_tokens_if_current(
        pool,
        &config.id,
        owner,
        &config.updated_at,
        &refreshed.access_token,
        &refreshed.refresh_token,
        &utc_now_string(),
    )
    .await
    .map_err(FetchRunError::App)?;
    let Some(updated) = updated else {
        return Err(FetchRunError::Query(MultiplierQueryError {
            code: "config_changed".into(),
            message: "刷新 token 期间配置已编辑或停用，本轮未更新账号倍率".into(),
            http_status: None,
        }));
    };
    *config = updated;
    tracing::info!(config_id = %config.id, "倍率监控 token 已自动刷新");
    multiplier_query::fetch_all(&config.url, &config.token, settings)
        .await
        .map_err(FetchRunError::Query)
}

async fn finish_query_failure(
    pool: &SqlitePool,
    config: &MultiplierMonitorConfig,
    owner: &str,
    started_at: &str,
    started: std::time::Instant,
    error: MultiplierQueryError,
) -> Result<MultiplierMonitorCheckResult, AppError> {
    let finished_at = utc_now_string();
    let run_id = Uuid::new_v4().to_string();
    let message = truncate_error(&error.message);
    let log = MultiplierMonitorLog {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        config_id: config.id.clone(),
        config_name: config.name.clone(),
        account_id: None,
        account_name: None,
        started_at: started_at.into(),
        finished_at: finished_at.clone(),
        duration_ms: Some(elapsed_millis(started)),
        result: "failed".into(),
        old_multiplier: None,
        remote_multiplier: None,
        error_code: Some(error.code),
        error_message: Some(message),
        http_status: error.http_status,
    };
    multiplier_monitor_dao::insert_log_and_release(pool, &log, &config.id, owner, &finished_at)
        .await?;
    Ok(summarize(&run_id, &[log]))
}

fn remote_map(
    remote: Vec<RemoteMultiplier>,
    multiplier_divisor: i64,
) -> Result<HashMap<String, ParsedRemoteRate>, MultiplierQueryError> {
    let mut result: HashMap<String, ParsedRemoteRate> = HashMap::with_capacity(remote.len());
    for item in remote {
        let parsed = parse_remote_rate(&item, multiplier_divisor);
        if let Some(existing) = result.get(&item.key) {
            if existing.decimal != parsed.decimal || existing.error != parsed.error {
                return Err(MultiplierQueryError {
                    code: "duplicate_key_conflict".into(),
                    message: "远端返回了倍率不一致的重复 key".into(),
                    http_status: None,
                });
            }
            continue;
        }
        result.insert(item.key, parsed);
    }
    Ok(result)
}

fn parse_remote_rate(item: &RemoteMultiplier, multiplier_divisor: i64) -> ParsedRemoteRate {
    if let Some(error) = item.rate_error.as_deref() {
        return ParsedRemoteRate {
            decimal: None,
            value: None,
            error: Some(error.into()),
        };
    }
    let Some(raw) = item.rate_multiplier.as_deref() else {
        return ParsedRemoteRate {
            decimal: None,
            value: None,
            error: Some("远端未返回 rate_multiplier".into()),
        };
    };
    let Ok(decimal) = Decimal::from_str(raw) else {
        return ParsedRemoteRate {
            decimal: None,
            value: None,
            error: Some("rate_multiplier 不是有效十进制数".into()),
        };
    };
    if !(1..=MAX_MULTIPLIER_DIVISOR).contains(&multiplier_divisor) {
        return ParsedRemoteRate {
            decimal: None,
            value: None,
            error: Some("倍率除数配置无效".into()),
        };
    }
    let normalized = (decimal / Decimal::from(multiplier_divisor)).normalize();
    if normalized < Decimal::new(1, 2) || normalized > Decimal::new(99, 2) {
        return ParsedRemoteRate {
            decimal: None,
            value: normalized.to_f64(),
            error: Some(format!(
                "rate_multiplier 除以 {multiplier_divisor} 后必须在 0.01～0.99 之间"
            )),
        };
    }
    if normalized.scale() > 2 {
        return ParsedRemoteRate {
            decimal: None,
            value: normalized.to_f64(),
            error: Some(format!(
                "rate_multiplier 除以 {multiplier_divisor} 后最多保留两位小数"
            )),
        };
    }
    ParsedRemoteRate {
        decimal: Some(normalized),
        value: normalized.to_f64(),
        error: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn account_log(
    run_id: &str,
    config: &MultiplierMonitorConfig,
    account_id: Option<String>,
    account_name: Option<String>,
    started_at: &str,
    finished_at: &str,
    duration_ms: i64,
    result: &str,
    old_multiplier: Option<f64>,
    remote_multiplier: Option<f64>,
    error_code: Option<&str>,
    error_message: Option<&str>,
) -> MultiplierMonitorLog {
    MultiplierMonitorLog {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.into(),
        config_id: config.id.clone(),
        config_name: config.name.clone(),
        account_id,
        account_name,
        started_at: started_at.into(),
        finished_at: finished_at.into(),
        duration_ms: Some(duration_ms),
        result: result.into(),
        old_multiplier,
        remote_multiplier,
        error_code: error_code.map(str::to_owned),
        error_message: error_message.map(truncate_error),
        http_status: None,
    }
}

fn summarize(run_id: &str, logs: &[MultiplierMonitorLog]) -> MultiplierMonitorCheckResult {
    MultiplierMonitorCheckResult {
        run_id: run_id.into(),
        total: logs.len() as i64,
        updated: logs.iter().filter(|log| log.result == "updated").count() as i64,
        unchanged: logs.iter().filter(|log| log.result == "unchanged").count() as i64,
        issues: logs
            .iter()
            .filter(|log| !matches!(log.result.as_str(), "updated" | "unchanged" | "failed"))
            .count() as i64,
        failed: logs.iter().filter(|log| log.result == "failed").count() as i64,
    }
}

async fn to_view(
    pool: &SqlitePool,
    config: MultiplierMonitorConfig,
    now: &str,
) -> Result<MultiplierMonitorConfigView, AppError> {
    let stats = multiplier_monitor_dao::latest_run_stats(pool, &config.id).await?;
    Ok(MultiplierMonitorConfigView {
        id: config.id,
        name: config.name,
        url: config.url,
        account_ids: parse_account_ids(&config.account_ids)?,
        multiplier_divisor: config.multiplier_divisor,
        enabled: config.enabled,
        has_token: !config.token.is_empty(),
        has_refresh_token: !config.refresh_token.is_empty(),
        last_started_at: config.last_started_at,
        last_finished_at: config.last_finished_at,
        running: config
            .lease_until
            .as_deref()
            .is_some_and(|lease_until| lease_until > now),
        last_run: stats.map(|value| MultiplierMonitorRunSummary {
            run_id: value.run_id,
            started_at: value.started_at,
            finished_at: value.finished_at,
            total: value.total,
            updated: value.updated,
            unchanged: value.unchanged,
            issues: value.issues,
            failed: value.failed,
        }),
        created_at: config.created_at,
        updated_at: config.updated_at,
    })
}

fn validate_name(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::BadRequest("配置名称不能为空".into()));
    }
    if value.chars().count() > 100 {
        return Err(AppError::BadRequest("配置名称不能超过 100 个字符".into()));
    }
    Ok(value.into())
}

fn validate_url(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    multiplier_query::normalize_base_url(value).map_err(|error| AppError::BadRequest(error.message))
}

fn normalize_token(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    let value = value
        .get(..7)
        .filter(|prefix| prefix.eq_ignore_ascii_case("Bearer "))
        .map(|_| value[7..].trim())
        .unwrap_or(value);
    if value.is_empty() {
        return Err(AppError::BadRequest("查询 token 不能为空".into()));
    }
    Ok(value.into())
}

fn normalize_refresh_token(value: &str) -> Result<String, AppError> {
    Ok(value.trim().into())
}

fn validate_multiplier_divisor(value: i64) -> Result<i64, AppError> {
    if !(1..=MAX_MULTIPLIER_DIVISOR).contains(&value) {
        return Err(AppError::BadRequest(format!(
            "倍率除数必须是 1～{MAX_MULTIPLIER_DIVISOR} 的整数"
        )));
    }
    Ok(value)
}

fn normalize_account_ids(values: Vec<String>) -> Result<Vec<String>, AppError> {
    let mut result = Vec::new();
    for value in values {
        let value = value.trim();
        if !value.is_empty() && !result.iter().any(|existing| existing == value) {
            result.push(value.to_owned());
        }
    }
    if result.is_empty() {
        return Err(AppError::BadRequest("请至少选择一个账号".into()));
    }
    if result.len() > MAX_ACCOUNT_COUNT {
        return Err(AppError::BadRequest("选择的账号数量超过上限".into()));
    }
    Ok(result)
}

fn parse_account_ids(raw: &str) -> Result<Vec<String>, AppError> {
    serde_json::from_str(raw).map_err(|_| AppError::Internal("倍率监控配置的账号列表无效".into()))
}

async fn validate_accounts(pool: &SqlitePool, account_ids: &[String]) -> Result<(), AppError> {
    let accounts = multiplier_monitor_dao::get_accounts_by_ids(pool, account_ids).await?;
    if accounts.len() != account_ids.len() {
        return Err(AppError::BadRequest("所选账号中存在已删除的账号".into()));
    }
    Ok(())
}

async fn validate_enabled_conflicts(
    pool: &SqlitePool,
    current_id: Option<&str>,
    enabled: bool,
    account_ids: &[String],
) -> Result<(), AppError> {
    if !enabled {
        return Ok(());
    }
    for config in multiplier_monitor_dao::list_configs(pool).await? {
        if !config.enabled || current_id == Some(config.id.as_str()) {
            continue;
        }
        let existing = parse_account_ids(&config.account_ids)?;
        if account_ids.iter().any(|id| existing.contains(id)) {
            return Err(AppError::BadRequest(format!(
                "所选账号已被启用的配置“{}”监控",
                config.name
            )));
        }
    }
    Ok(())
}

fn truncate_error(value: &str) -> String {
    value.chars().take(MAX_ERROR_LENGTH).collect()
}

#[cfg(test)]
mod tests {
    use super::{normalize_token, parse_remote_rate, validate_multiplier_divisor};
    use crate::service::multiplier_query::RemoteMultiplier;

    #[test]
    fn strips_optional_bearer_prefix_before_storage() {
        assert_eq!(normalize_token("Bearer abc").unwrap(), "abc");
        assert_eq!(normalize_token("abc").unwrap(), "abc");
    }

    #[test]
    fn validates_remote_decimal_range_and_scale() {
        let valid = parse_remote_rate(
            &RemoteMultiplier {
                key: "key".into(),
                rate_multiplier: Some("0.300".into()),
                rate_error: None,
            },
            1,
        );
        assert!(valid.error.is_none());
        let invalid = parse_remote_rate(
            &RemoteMultiplier {
                key: "key".into(),
                rate_multiplier: Some("0.301".into()),
                rate_error: None,
            },
            1,
        );
        assert!(invalid.error.is_some());
    }

    #[test]
    fn divides_remote_rate_before_validation() {
        let parsed = parse_remote_rate(
            &RemoteMultiplier {
                key: "key".into(),
                rate_multiplier: Some("3".into()),
                rate_error: None,
            },
            10,
        );
        assert!(parsed.error.is_none());
        assert_eq!(parsed.decimal.unwrap().to_string(), "0.3");

        let too_precise = parse_remote_rate(
            &RemoteMultiplier {
                key: "key".into(),
                rate_multiplier: Some("3.01".into()),
                rate_error: None,
            },
            10,
        );
        assert!(too_precise.error.is_some());
    }

    #[test]
    fn validates_multiplier_divisor_range() {
        assert_eq!(validate_multiplier_divisor(1).unwrap(), 1);
        assert_eq!(validate_multiplier_divisor(10_000).unwrap(), 10_000);
        assert!(validate_multiplier_divisor(0).is_err());
        assert!(validate_multiplier_divisor(10_001).is_err());
    }
}
