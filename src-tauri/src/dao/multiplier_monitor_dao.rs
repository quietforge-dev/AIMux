use sqlx::{Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

use crate::{
    error::AppError,
    model::{
        account::Account,
        multiplier_monitor::{LatestRunStats, MultiplierMonitorConfig, MultiplierMonitorLog},
    },
    schema::multiplier_monitor_schema::MultiplierMonitorAccountOption,
};

pub struct AccountMultiplierChange {
    pub log_index: usize,
    pub account_id: String,
    pub expected_key: String,
    pub expected_multiplier: f64,
    pub next_multiplier: f64,
}

pub async fn list_configs(pool: &SqlitePool) -> Result<Vec<MultiplierMonitorConfig>, AppError> {
    Ok(sqlx::query_as::<_, MultiplierMonitorConfig>(
        "SELECT * FROM multiplier_monitor_configs ORDER BY lower(name), id",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn get_config(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<MultiplierMonitorConfig>, AppError> {
    Ok(sqlx::query_as::<_, MultiplierMonitorConfig>(
        "SELECT * FROM multiplier_monitor_configs WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

pub async fn create_config(
    pool: &SqlitePool,
    name: &str,
    url: &str,
    token: &str,
    account_ids: &str,
    enabled: bool,
    now: &str,
) -> Result<MultiplierMonitorConfig, AppError> {
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO multiplier_monitor_configs(id,name,url,token,account_ids,enabled,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?)")
        .bind(&id)
        .bind(name)
        .bind(url)
        .bind(token)
        .bind(account_ids)
        .bind(enabled)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
    get_config(pool, &id)
        .await?
        .ok_or_else(|| AppError::Internal("创建倍率监控配置后读取失败".into()))
}

#[allow(clippy::too_many_arguments)]
pub async fn update_config(
    pool: &SqlitePool,
    id: &str,
    name: &str,
    url: &str,
    token: &str,
    account_ids: &str,
    enabled: bool,
    reset_schedule: bool,
    now: &str,
) -> Result<Option<MultiplierMonitorConfig>, AppError> {
    let result = sqlx::query(
        "UPDATE multiplier_monitor_configs SET name=?,url=?,token=?,account_ids=?,enabled=?,last_started_at=CASE WHEN ? THEN NULL ELSE last_started_at END,updated_at=? WHERE id=?",
    )
    .bind(name)
    .bind(url)
    .bind(token)
    .bind(account_ids)
    .bind(enabled)
    .bind(reset_schedule)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    get_config(pool, id).await
}

pub async fn toggle_config(
    pool: &SqlitePool,
    id: &str,
    now: &str,
) -> Result<Option<MultiplierMonitorConfig>, AppError> {
    let result = sqlx::query("UPDATE multiplier_monitor_configs SET last_started_at=CASE WHEN enabled=0 THEN NULL ELSE last_started_at END,enabled=CASE WHEN enabled=1 THEN 0 ELSE 1 END,updated_at=? WHERE id=?")
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    get_config(pool, id).await
}

pub async fn list_account_options(
    pool: &SqlitePool,
) -> Result<Vec<MultiplierMonitorAccountOption>, AppError> {
    let rows = sqlx::query_as::<_, (String, String, String, String, f64)>(
        "SELECT id,name,type,status,multiplier FROM accounts ORDER BY CASE WHEN status='active' THEN 0 ELSE 1 END, lower(name), id",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, name, account_type, status, multiplier)| MultiplierMonitorAccountOption {
                id,
                name,
                account_type,
                status,
                multiplier,
            },
        )
        .collect())
}

pub async fn get_accounts_by_ids(
    pool: &SqlitePool,
    ids: &[String],
) -> Result<Vec<Account>, AppError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat("?")
        .take(ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!("SELECT * FROM accounts WHERE id IN ({placeholders})");
    let mut query = sqlx::query_as::<_, Account>(&sql);
    for id in ids {
        query = query.bind(id);
    }
    Ok(query.fetch_all(pool).await?)
}

pub async fn latest_run_stats(
    pool: &SqlitePool,
    config_id: &str,
) -> Result<Option<LatestRunStats>, AppError> {
    Ok(sqlx::query_as::<_, LatestRunStats>(
        r#"
        WITH latest AS (
            SELECT run_id
            FROM multiplier_monitor_logs
            WHERE config_id = ?
            ORDER BY started_at DESC, id DESC
            LIMIT 1
        )
        SELECT logs.run_id,
               MIN(logs.started_at) AS started_at,
               MAX(logs.finished_at) AS finished_at,
               COUNT(*) AS total,
               SUM(CASE WHEN logs.result='updated' THEN 1 ELSE 0 END) AS updated,
               SUM(CASE WHEN logs.result='unchanged' THEN 1 ELSE 0 END) AS unchanged,
               SUM(CASE WHEN logs.result NOT IN ('updated','unchanged','failed') THEN 1 ELSE 0 END) AS issues,
               SUM(CASE WHEN logs.result='failed' THEN 1 ELSE 0 END) AS failed
        FROM multiplier_monitor_logs logs
        JOIN latest ON latest.run_id = logs.run_id
        GROUP BY logs.run_id
        "#,
    )
    .bind(config_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn list_due_ids(
    pool: &SqlitePool,
    due_before: &str,
    now: &str,
) -> Result<Vec<String>, AppError> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT id FROM multiplier_monitor_configs WHERE enabled=1 AND (last_started_at IS NULL OR last_started_at<=?) AND (lease_until IS NULL OR lease_until<=?) ORDER BY COALESCE(last_started_at,''),id",
    )
    .bind(due_before)
    .bind(now)
    .fetch_all(pool)
    .await?)
}

#[allow(clippy::too_many_arguments)]
pub async fn claim_config(
    pool: &SqlitePool,
    id: &str,
    owner: &str,
    now: &str,
    lease_until: &str,
    due_before: &str,
    force: bool,
) -> Result<Option<MultiplierMonitorConfig>, AppError> {
    let result = sqlx::query(
        "UPDATE multiplier_monitor_configs SET lease_owner=?,lease_until=?,last_started_at=? WHERE id=? AND enabled=1 AND (lease_until IS NULL OR lease_until<=?) AND (? OR last_started_at IS NULL OR last_started_at<=?)",
    )
    .bind(owner)
    .bind(lease_until)
    .bind(now)
    .bind(id)
    .bind(now)
    .bind(force)
    .bind(due_before)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    get_config(pool, id).await
}

pub async fn release_config(
    pool: &SqlitePool,
    id: &str,
    owner: &str,
    finished_at: &str,
) -> Result<(), AppError> {
    sqlx::query("UPDATE multiplier_monitor_configs SET lease_owner=NULL,lease_until=NULL,last_finished_at=? WHERE id=? AND lease_owner=?")
        .bind(finished_at)
        .bind(id)
        .bind(owner)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn insert_log(pool: &SqlitePool, log: &MultiplierMonitorLog) -> Result<(), AppError> {
    let mut transaction = pool.begin().await?;
    insert_log_tx(&mut transaction, log).await?;
    transaction.commit().await?;
    Ok(())
}

pub async fn insert_log_and_release(
    pool: &SqlitePool,
    log: &MultiplierMonitorLog,
    config_id: &str,
    owner: &str,
    finished_at: &str,
) -> Result<(), AppError> {
    let mut transaction = pool.begin().await?;
    insert_log_tx(&mut transaction, log).await?;
    sqlx::query("UPDATE multiplier_monitor_configs SET lease_owner=NULL,lease_until=NULL,last_finished_at=? WHERE id=? AND lease_owner=?")
        .bind(finished_at)
        .bind(config_id)
        .bind(owner)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(())
}

pub async fn complete_run(
    pool: &SqlitePool,
    config: &MultiplierMonitorConfig,
    owner: &str,
    finished_at: &str,
    logs: &mut [MultiplierMonitorLog],
    changes: &[AccountMultiplierChange],
) -> Result<bool, AppError> {
    let mut transaction = pool.begin().await?;
    let current: Option<(bool, String)> =
        sqlx::query_as("SELECT enabled,updated_at FROM multiplier_monitor_configs WHERE id=?")
            .bind(&config.id)
            .fetch_optional(&mut *transaction)
            .await?;
    if !matches!(current, Some((true, ref updated_at)) if updated_at == &config.updated_at) {
        transaction.rollback().await?;
        return Ok(false);
    }

    for change in changes {
        let result = sqlx::query("UPDATE accounts SET multiplier=?,updated_at=? WHERE id=? AND api_key_encrypted=? AND multiplier=?")
            .bind(change.next_multiplier)
            .bind(finished_at)
            .bind(&change.account_id)
            .bind(&change.expected_key)
            .bind(change.expected_multiplier)
            .execute(&mut *transaction)
            .await?;
        if result.rows_affected() == 0 {
            if let Some(log) = logs.get_mut(change.log_index) {
                log.result = "conflict".into();
                log.error_code = Some("account_changed".into());
                log.error_message = Some("检查期间账号密钥或倍率已被修改，未覆盖新值".into());
            }
        }
    }
    for log in logs.iter() {
        insert_log_tx(&mut transaction, log).await?;
    }
    sqlx::query("UPDATE multiplier_monitor_configs SET lease_owner=NULL,lease_until=NULL,last_finished_at=? WHERE id=? AND lease_owner=?")
        .bind(finished_at)
        .bind(&config.id)
        .bind(owner)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(true)
}

async fn insert_log_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    log: &MultiplierMonitorLog,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO multiplier_monitor_logs(id,run_id,config_id,config_name,account_id,account_name,started_at,finished_at,duration_ms,result,old_multiplier,remote_multiplier,error_code,error_message,http_status) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
        .bind(&log.id)
        .bind(&log.run_id)
        .bind(&log.config_id)
        .bind(&log.config_name)
        .bind(&log.account_id)
        .bind(&log.account_name)
        .bind(&log.started_at)
        .bind(&log.finished_at)
        .bind(log.duration_ms)
        .bind(&log.result)
        .bind(log.old_multiplier)
        .bind(log.remote_multiplier)
        .bind(&log.error_code)
        .bind(&log.error_message)
        .bind(log.http_status)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

pub async fn list_logs(
    pool: &SqlitePool,
    config_id: &str,
    offset: i64,
    limit: i64,
    result: Option<&str>,
) -> Result<(Vec<MultiplierMonitorLog>, i64), AppError> {
    let (items, total) = if let Some(result) = result.filter(|value| !value.trim().is_empty()) {
        let items = sqlx::query_as::<_, MultiplierMonitorLog>("SELECT * FROM multiplier_monitor_logs WHERE config_id=? AND result=? ORDER BY started_at DESC,id DESC LIMIT ? OFFSET ?")
            .bind(config_id)
            .bind(result)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM multiplier_monitor_logs WHERE config_id=? AND result=?",
        )
        .bind(config_id)
        .bind(result)
        .fetch_one(pool)
        .await?;
        (items, total)
    } else {
        let items = sqlx::query_as::<_, MultiplierMonitorLog>("SELECT * FROM multiplier_monitor_logs WHERE config_id=? ORDER BY started_at DESC,id DESC LIMIT ? OFFSET ?")
            .bind(config_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM multiplier_monitor_logs WHERE config_id=?",
        )
        .bind(config_id)
        .fetch_one(pool)
        .await?;
        (items, total)
    };
    Ok((items, total))
}
