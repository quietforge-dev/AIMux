use std::{str::FromStr, time::Duration};

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
        MultiplierMonitorCheckResult, MultiplierMonitorConfigView, MultiplierMonitorUpdate,
    },
    service::multiplier_query::{self, MultiplierQueryError, RemoteMultiplier},
    utils::time::{elapsed_millis, utc_datetime_string, utc_now_string},
};

const MAX_MULTIPLIER_DIVISOR: i64 = 10_000;
const LEASE_MINUTES: i64 = 10;
const SCHEDULE_INTERVAL_HOURS: i64 = 1;
const QUERY_TIMEOUT: Duration = Duration::from_secs(60);

pub async fn list(pool: &SqlitePool) -> Result<Vec<MultiplierMonitorConfigView>, AppError> {
    let now = utc_now_string();
    multiplier_monitor_dao::list_entries(pool)
        .await?
        .iter()
        .map(|entry| view(entry, &now))
        .collect()
}

pub async fn get_view(
    pool: &SqlitePool,
    id: &str,
) -> Result<MultiplierMonitorConfigView, AppError> {
    let entry = multiplier_monitor_dao::get_entry(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("账号不存在".into()))?;
    view(&entry, &utc_now_string())
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    payload: MultiplierMonitorUpdate,
) -> Result<MultiplierMonitorConfigView, AppError> {
    let current = multiplier_monitor_dao::get_config(pool, id).await?;
    let platform = payload
        .platform_type
        .as_deref()
        .or(current.as_ref().map(|c| c.platform_type.as_str()))
        .unwrap_or("sub2api");
    let group = if platform == "newapi" {
        payload
            .new_api_group
            .as_deref()
            .or(current.as_ref().and_then(|c| c.new_api_group.as_deref()))
    } else {
        None
    };
    let divisor = payload
        .multiplier_divisor
        .or(current.as_ref().map(|c| c.multiplier_divisor))
        .unwrap_or(1);
    let sync = payload
        .sync_upstream_multiplier
        .or(current.as_ref().map(|c| c.sync_upstream_multiplier))
        .unwrap_or(false);
    let enabled = payload
        .enabled
        .or(current.as_ref().map(|c| c.enabled))
        .unwrap_or(false);
    save(pool, id, platform, group, sync, divisor, enabled).await
}

async fn save(
    pool: &SqlitePool,
    account_id: &str,
    platform: &str,
    group: Option<&str>,
    sync: bool,
    divisor: i64,
    enabled: bool,
) -> Result<MultiplierMonitorConfigView, AppError> {
    validate(account_id, platform, group, divisor)?;
    multiplier_monitor_dao::get_entry(pool, account_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("账号不存在".into()))?;
    multiplier_monitor_dao::upsert_config(
        pool,
        account_id,
        platform,
        group,
        sync,
        divisor,
        enabled,
        &utc_now_string(),
    )
    .await?;
    get_view(pool, account_id).await
}

pub async fn toggle(pool: &SqlitePool, id: &str) -> Result<MultiplierMonitorConfigView, AppError> {
    let now = utc_now_string();
    if multiplier_monitor_dao::get_config(pool, id)
        .await?
        .is_some()
    {
        multiplier_monitor_dao::toggle_config(pool, id, &now).await?;
    } else {
        return Err(AppError::BadRequest("请先配置上游平台后再开启监控".into()));
    }
    get_view(pool, id).await
}

pub async fn check_now(
    pool: &SqlitePool,
    settings: &Settings,
    id: &str,
) -> Result<MultiplierMonitorCheckResult, AppError> {
    let owner = format!("manual-{}", Uuid::new_v4());
    let cfg = claim(pool, id, &owner, true)
        .await?
        .ok_or_else(|| AppError::BadRequest("监控已关闭或正在检查中".into()))?;
    run_claimed(pool, settings, cfg, &owner).await
}

pub async fn run_due(pool: &SqlitePool, settings: &Settings, worker: &str) -> Result<(), AppError> {
    let now = Utc::now();
    let ids = multiplier_monitor_dao::list_due_ids(pool, &due_time(now), &utc_datetime_string(now))
        .await?;
    let mut pending = stream::iter(ids)
        .map(|id| async move {
            let owner = format!("{worker}-{}", Uuid::new_v4());
            if let Some(config) = claim(pool, &id, &owner, false).await? {
                if let Err(error) = run_claimed(pool, settings, config, &owner).await {
                    tracing::error!(account_id = %id, %error, "倍率监控执行失败");
                }
            }
            Ok::<(), AppError>(())
        })
        .buffer_unordered(2);
    while let Some(result) = pending.next().await {
        if let Err(error) = result {
            tracing::error!(%error, "倍率监控任务领取失败");
        }
    }
    Ok(())
}

pub async fn newapi_groups(
    pool: &SqlitePool,
    settings: &Settings,
    account_id: &str,
) -> Result<Vec<multiplier_query::NewApiGroup>, AppError> {
    let account = multiplier_monitor_dao::get_account(pool, account_id)
        .await?
        .ok_or_else(|| AppError::NotFound("账号不存在".into()))?;
    multiplier_query::fetch_newapi_groups(&account.base_url, settings)
        .await
        .map_err(|e| AppError::BadRequest(e.message))
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
        &utc_datetime_string(now + ChronoDuration::minutes(LEASE_MINUTES)),
        &due_time(now),
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
    let entry = multiplier_monitor_dao::get_entry(pool, &config.account_id)
        .await?
        .ok_or_else(|| AppError::NotFound("账号不存在".into()))?;
    let started_at = config
        .last_started_at
        .clone()
        .unwrap_or_else(utc_now_string);
    let timer = std::time::Instant::now();
    let query = tokio::time::timeout(QUERY_TIMEOUT, fetch_rate(settings, &entry, &config)).await;
    let run_id = Uuid::new_v4().to_string();
    let finished_at = utc_now_string();
    let mut log = MultiplierMonitorLog {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        config_id: config.id.clone(),
        config_name: entry.account_name.clone(),
        account_id: Some(entry.account_id.clone()),
        account_name: Some(entry.account_name.clone()),
        started_at: started_at.clone(),
        finished_at: finished_at.clone(),
        duration_ms: Some(elapsed_millis(timer)),
        result: "failed".into(),
        old_multiplier: Some(entry.account_multiplier),
        remote_multiplier: None,
        error_code: None,
        error_message: None,
        http_status: None,
    };
    let mut change = None;
    match query {
        Err(_) => {
            log.error_code = Some("query_timeout".into());
            log.error_message = Some("倍率查询超过 60 秒限制".into());
        }
        Ok(Err(error)) => {
            log.error_code = Some(error.code);
            log.error_message = Some(error.message);
            log.http_status = error.http_status;
        }
        Ok(Ok(remote)) => match parse_rate(&remote, config.multiplier_divisor) {
            Err(message) => {
                log.error_code = Some("invalid_rate".into());
                log.error_message = Some(message);
            }
            Ok(rate) => {
                log.remote_multiplier = Some(rate);
                if (rate - entry.account_multiplier).abs() < 0.000001 {
                    log.result = "unchanged".into();
                } else if config.sync_upstream_multiplier {
                    log.result = "updated".into();
                    change = Some(AccountMultiplierChange {
                        log_index: 0,
                        account_id: entry.account_id.clone(),
                        expected_key: entry.api_key.clone(),
                        expected_multiplier: entry.account_multiplier,
                        next_multiplier: rate,
                    });
                } else {
                    log.result = "observed".into();
                }
            }
        },
    }
    let ok = log.result != "failed";
    let error = log.error_message.clone();
    multiplier_monitor_dao::finish_run(
        pool,
        &config,
        owner,
        &finished_at,
        &mut log,
        change.as_ref(),
        ok,
        error.as_deref(),
    )
    .await?;
    let updated = i64::from(log.result == "updated");
    let unchanged = i64::from(matches!(log.result.as_str(), "unchanged" | "observed"));
    let issues = i64::from(matches!(log.result.as_str(), "conflict"));
    let failed = i64::from(log.result == "failed");
    Ok(MultiplierMonitorCheckResult {
        run_id,
        total: 1,
        updated,
        unchanged,
        issues,
        failed,
    })
}

async fn fetch_rate(
    settings: &Settings,
    account: &crate::model::multiplier_monitor::MultiplierMonitorEntry,
    config: &MultiplierMonitorConfig,
) -> Result<RemoteMultiplier, MultiplierQueryError> {
    if account.account_status != "active" {
        return Err(MultiplierQueryError {
            code: "account_disabled".into(),
            message: "账号已停用".into(),
            http_status: None,
        });
    }
    match config.platform_type.as_str() {
        "sub2api" => {
            multiplier_query::fetch_sub2api_billing(&account.base_url, &account.api_key, settings)
                .await
        }
        "newapi" => {
            let group = config
                .new_api_group
                .as_deref()
                .ok_or_else(|| MultiplierQueryError {
                    code: "group_required".into(),
                    message: "New API 未配置组".into(),
                    http_status: None,
                })?;
            let rate =
                multiplier_query::fetch_newapi_rate(&account.base_url, group, settings).await?;
            Ok(RemoteMultiplier {
                key: account.api_key.clone(),
                rate_multiplier: Some(rate),
                rate_error: None,
            })
        }
        _ => Err(MultiplierQueryError {
            code: "unsupported_platform".into(),
            message: "不支持的上游平台".into(),
            http_status: None,
        }),
    }
}

fn parse_rate(remote: &RemoteMultiplier, divisor: i64) -> Result<f64, String> {
    if let Some(error) = remote.rate_error.as_deref() {
        return Err(error.to_owned());
    }
    let raw = remote
        .rate_multiplier
        .as_deref()
        .ok_or_else(|| "上游未返回有效倍率".to_owned())?;
    let value = Decimal::from_str(raw).map_err(|_| "上游倍率不是有效数字".to_owned())?
        / Decimal::from(divisor);
    if value < Decimal::new(1, 2) || value > Decimal::new(99, 2) || value.scale() > 2 {
        return Err(format!(
            "倍率除以 {divisor} 后必须在 0.01～0.99 且最多两位小数"
        ));
    }
    value.to_f64().ok_or_else(|| "上游倍率无法转换".to_owned())
}

fn view(
    entry: &crate::model::multiplier_monitor::MultiplierMonitorEntry,
    now: &str,
) -> Result<MultiplierMonitorConfigView, AppError> {
    Ok(MultiplierMonitorConfigView {
        id: entry
            .config_id
            .clone()
            .unwrap_or_else(|| entry.account_id.clone()),
        account_id: entry.account_id.clone(),
        account_name: entry.account_name.clone(),
        account_type: entry.account_type.clone(),
        account_status: entry.account_status.clone(),
        account_multiplier: entry.account_multiplier,
        configured: entry.config_id.is_some(),
        platform_type: entry
            .platform_type
            .clone()
            .unwrap_or_else(|| "sub2api".into()),
        new_api_group: entry.new_api_group.clone(),
        sync_upstream_multiplier: entry.sync_upstream_multiplier.unwrap_or(false),
        multiplier_divisor: entry.multiplier_divisor.unwrap_or(1),
        enabled: entry.enabled.unwrap_or(false),
        last_started_at: entry.last_started_at.clone(),
        last_finished_at: entry.last_finished_at.clone(),
        last_success_at: entry.last_success_at.clone(),
        last_error: entry.last_error.clone(),
        last_remote_multiplier: entry.last_remote_multiplier,
        last_result: entry.last_result.clone(),
        running: entry
            .lease_until
            .as_deref()
            .is_some_and(|until| until > now),
        last_run: None,
        created_at: entry
            .config_created_at
            .clone()
            .unwrap_or_else(|| now.into()),
        updated_at: entry
            .config_updated_at
            .clone()
            .unwrap_or_else(|| now.into()),
    })
}

fn validate(
    account_id: &str,
    platform: &str,
    group: Option<&str>,
    divisor: i64,
) -> Result<(), AppError> {
    if account_id.trim().is_empty() {
        return Err(AppError::BadRequest("账号不能为空".into()));
    }
    if !matches!(platform, "sub2api" | "newapi") {
        return Err(AppError::BadRequest(
            "平台类型只能是 sub2api 或 newapi".into(),
        ));
    }
    if platform == "newapi" && group.map(str::trim).unwrap_or("").is_empty() {
        return Err(AppError::BadRequest("New API 必须选择组".into()));
    }
    if !(1..=MAX_MULTIPLIER_DIVISOR).contains(&divisor) {
        return Err(AppError::BadRequest(
            "倍率除数必须是 1～10000 的整数".into(),
        ));
    }
    Ok(())
}

fn due_time(now: chrono::DateTime<Utc>) -> String {
    utc_datetime_string(now - ChronoDuration::hours(SCHEDULE_INTERVAL_HOURS))
}
