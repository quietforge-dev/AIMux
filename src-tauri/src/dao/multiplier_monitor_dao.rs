use crate::database::UserDb;
use sqlx::{Sqlite, Transaction};

use crate::{
    error::AppError,
    model::{
        account::Account,
        multiplier_monitor::{
            MultiplierMonitorConfig, MultiplierMonitorEntry, MultiplierMonitorLog,
        },
    },
};

pub struct AccountMultiplierChange {
    pub log_index: usize,
    pub account_id: String,
    pub expected_key: String,
    pub expected_multiplier: f64,
    pub next_multiplier: f64,
}

pub async fn list_entries(pool: &UserDb) -> Result<Vec<MultiplierMonitorEntry>, AppError> {
    Ok(sqlx::query_as::<_, MultiplierMonitorEntry>(r#"SELECT a.id account_id,a.name account_name,a.type account_type,a.base_url,a.api_key_encrypted api_key,a.status account_status,a.multiplier account_multiplier,mm.id config_id,mm.platform_type,mm.new_api_group,mm.sync_upstream_multiplier,mm.multiplier_divisor,mm.enabled,mm.last_started_at,mm.last_finished_at,mm.lease_owner,mm.lease_until,mm.last_success_at,mm.last_error,mm.last_remote_multiplier,mm.last_result,mm.created_at config_created_at,mm.updated_at config_updated_at FROM accounts a LEFT JOIN multiplier_monitor_configs mm ON mm.account_id=a.id AND mm.user_id=a.user_id WHERE a.user_id=?1 ORDER BY CASE WHEN a.status='active' THEN 0 ELSE 1 END,lower(a.name),a.id"#).bind(pool.user_id()).fetch_all(pool.raw()).await?)
}

pub async fn get_entry(
    pool: &UserDb,
    account_id: &str,
) -> Result<Option<MultiplierMonitorEntry>, AppError> {
    Ok(sqlx::query_as::<_, MultiplierMonitorEntry>(r#"SELECT a.id account_id,a.name account_name,a.type account_type,a.base_url,a.api_key_encrypted api_key,a.status account_status,a.multiplier account_multiplier,mm.id config_id,mm.platform_type,mm.new_api_group,mm.sync_upstream_multiplier,mm.multiplier_divisor,mm.enabled,mm.last_started_at,mm.last_finished_at,mm.lease_owner,mm.lease_until,mm.last_success_at,mm.last_error,mm.last_remote_multiplier,mm.last_result,mm.created_at config_created_at,mm.updated_at config_updated_at FROM accounts a LEFT JOIN multiplier_monitor_configs mm ON mm.account_id=a.id AND mm.user_id=a.user_id WHERE a.user_id=?1 AND a.id=?2"#).bind(pool.user_id()).bind(account_id).fetch_optional(pool.raw()).await?)
}

pub async fn get_config(
    pool: &UserDb,
    id: &str,
) -> Result<Option<MultiplierMonitorConfig>, AppError> {
    Ok(
        sqlx::query_as("SELECT * FROM multiplier_monitor_configs WHERE user_id=?1 AND id=?2")
            .bind(pool.user_id())
            .bind(id)
            .fetch_optional(pool.raw())
            .await?,
    )
}
pub async fn upsert_config(
    pool: &UserDb,
    account_id: &str,
    platform: &str,
    group: Option<&str>,
    sync: bool,
    divisor: i64,
    enabled: bool,
    now: &str,
) -> Result<MultiplierMonitorConfig, AppError> {
    let id = account_id.to_owned();
    sqlx::query("INSERT INTO multiplier_monitor_configs(user_id,id,account_id,platform_type,new_api_group,sync_upstream_multiplier,multiplier_divisor,enabled,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) ON CONFLICT(id) DO UPDATE SET account_id=excluded.account_id,platform_type=excluded.platform_type,new_api_group=excluded.new_api_group,sync_upstream_multiplier=excluded.sync_upstream_multiplier,multiplier_divisor=excluded.multiplier_divisor,enabled=excluded.enabled,last_error=NULL,last_started_at=CASE WHEN multiplier_monitor_configs.platform_type IS NOT excluded.platform_type OR multiplier_monitor_configs.new_api_group IS NOT excluded.new_api_group OR multiplier_monitor_configs.multiplier_divisor IS NOT excluded.multiplier_divisor THEN NULL ELSE multiplier_monitor_configs.last_started_at END,updated_at=excluded.updated_at WHERE multiplier_monitor_configs.user_id=excluded.user_id").bind(pool.user_id())
        .bind(id).bind(account_id).bind(platform).bind(group).bind(sync).bind(divisor).bind(enabled).bind(now).bind(now).execute(pool.raw()).await?;
    sqlx::query_as("SELECT * FROM multiplier_monitor_configs WHERE user_id=?1 AND account_id=?2")
        .bind(pool.user_id())
        .bind(account_id)
        .fetch_one(pool.raw())
        .await
        .map_err(Into::into)
}

pub async fn toggle_config(
    pool: &UserDb,
    account_id: &str,
    now: &str,
) -> Result<MultiplierMonitorConfig, AppError> {
    let existing = sqlx::query_scalar::<_, String>(
        "SELECT id FROM multiplier_monitor_configs WHERE user_id=?1 AND account_id=?2",
    )
    .bind(pool.user_id())
    .bind(account_id)
    .fetch_optional(pool.raw())
    .await?;
    if let Some(id) = existing {
        sqlx::query("UPDATE multiplier_monitor_configs SET enabled=CASE WHEN enabled=1 THEN 0 ELSE 1 END,last_started_at=CASE WHEN enabled=1 THEN last_started_at ELSE NULL END,updated_at=?2 WHERE user_id=?1 AND id=?3").bind(pool.user_id()).bind(now).bind(id).execute(pool.raw()).await?;
    } else {
        return upsert_config(pool, account_id, "sub2api", None, false, 1, true, now).await;
    }
    sqlx::query_as("SELECT * FROM multiplier_monitor_configs WHERE user_id=?1 AND account_id=?2")
        .bind(pool.user_id())
        .bind(account_id)
        .fetch_one(pool.raw())
        .await
        .map_err(Into::into)
}

pub async fn get_account(pool: &UserDb, id: &str) -> Result<Option<Account>, AppError> {
    Ok(
        sqlx::query_as("SELECT * FROM accounts WHERE user_id=?1 AND id=?2")
            .bind(pool.user_id())
            .bind(id)
            .fetch_optional(pool.raw())
            .await?,
    )
}

pub async fn list_due_ids(
    pool: &UserDb,
    due_before: &str,
    now: &str,
) -> Result<Vec<String>, AppError> {
    Ok(sqlx::query_scalar("SELECT id FROM multiplier_monitor_configs WHERE user_id=?1 AND enabled=1 AND (last_started_at IS NULL OR last_started_at<=?2) AND (lease_until IS NULL OR lease_until<=?3) ORDER BY COALESCE(last_started_at,''),id").bind(pool.user_id()).bind(due_before).bind(now).fetch_all(pool.raw()).await?)
}
pub async fn claim_config(
    pool: &UserDb,
    id: &str,
    owner: &str,
    now: &str,
    lease_until: &str,
    due_before: &str,
    force: bool,
) -> Result<Option<MultiplierMonitorConfig>, AppError> {
    let r=sqlx::query("UPDATE multiplier_monitor_configs SET lease_owner=?2,lease_until=?3,last_started_at=?4 WHERE user_id=?1 AND id=?5 AND enabled=1 AND (lease_until IS NULL OR lease_until<=?6) AND (?7 OR last_started_at IS NULL OR last_started_at<=?8)").bind(pool.user_id()).bind(owner).bind(lease_until).bind(now).bind(id).bind(now).bind(force).bind(due_before).execute(pool.raw()).await?;
    if r.rows_affected() == 0 {
        return Ok(None);
    };
    get_config(pool, id).await
}
pub async fn release_config(
    pool: &UserDb,
    id: &str,
    owner: &str,
    finished: &str,
) -> Result<(), AppError> {
    sqlx::query("UPDATE multiplier_monitor_configs SET lease_owner=NULL,lease_until=NULL,last_finished_at=?2 WHERE user_id=?1 AND id=?3 AND lease_owner=?4").bind(pool.user_id()).bind(finished).bind(id).bind(owner).execute(pool.raw()).await?;
    Ok(())
}
pub async fn update_result(
    pool: &UserDb,
    id: &str,
    owner: &str,
    finished: &str,
    success: bool,
    error: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query("UPDATE multiplier_monitor_configs SET lease_owner=NULL,lease_until=NULL,last_finished_at=?2,last_success_at=CASE WHEN ?3 THEN ?4 ELSE last_success_at END,last_error=?5,updated_at=?6 WHERE user_id=?1 AND id=?7 AND lease_owner=?8").bind(pool.user_id()).bind(finished).bind(success).bind(finished).bind(error).bind(finished).bind(id).bind(owner).execute(pool.raw()).await?;
    Ok(())
}

pub async fn finish_run(
    pool: &UserDb,
    config: &MultiplierMonitorConfig,
    owner: &str,
    finished: &str,
    log: &mut MultiplierMonitorLog,
    change: Option<&AccountMultiplierChange>,
    success: bool,
    error: Option<&str>,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    let current: Option<(bool, String)> = sqlx::query_as(
        "SELECT enabled,updated_at FROM multiplier_monitor_configs WHERE user_id=?1 AND id=?2",
    )
    .bind(pool.user_id())
    .bind(&config.id)
    .fetch_optional(&mut *tx)
    .await?;
    if !matches!(current, Some((true, ref updated)) if updated == &config.updated_at) {
        log.result = "cancelled".into();
        log.error_code = Some("config_changed".into());
        log.error_message = Some("检查期间配置已被修改或停用".into());
    } else if let Some(change) = change {
        let result = sqlx::query("UPDATE accounts SET multiplier=?2,updated_at=?3 WHERE user_id=?1 AND id=?4 AND api_key_encrypted=?5 AND multiplier=?6").bind(pool.user_id()).bind(change.next_multiplier).bind(finished).bind(&change.account_id).bind(&change.expected_key).bind(change.expected_multiplier).execute(&mut *tx).await?;
        if result.rows_affected() == 0 {
            log.result = "conflict".into();
            log.error_code = Some("account_changed".into());
            log.error_message = Some("账号已被修改，未覆盖新倍率".into());
        }
    }
    insert_log_tx(pool.user_id(), &mut tx, log).await?;
    sqlx::query("UPDATE multiplier_monitor_configs SET lease_owner=NULL,lease_until=NULL,last_finished_at=?2,last_success_at=CASE WHEN ?3 THEN ?4 ELSE last_success_at END,last_error=?5,last_remote_multiplier=?6,last_result=?7,updated_at=?8 WHERE user_id=?1 AND id=?9 AND lease_owner=?10").bind(pool.user_id()).bind(finished).bind(success).bind(finished).bind(error.or(log.error_message.as_deref())).bind(log.remote_multiplier).bind(&log.result).bind(finished).bind(&config.id).bind(owner).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn update_account_multiplier(
    pool: &UserDb,
    change: &AccountMultiplierChange,
) -> Result<bool, AppError> {
    Ok(sqlx::query("UPDATE accounts SET multiplier=?2,updated_at=?3 WHERE user_id=?1 AND id=?4 AND api_key_encrypted=?5 AND multiplier=?6").bind(pool.user_id()).bind(change.next_multiplier).bind(crate::utils::time::utc_now_string()).bind(&change.account_id).bind(&change.expected_key).bind(change.expected_multiplier).execute(pool.raw()).await?.rows_affected()>0)
}
pub async fn insert_log(pool: &UserDb, log: &MultiplierMonitorLog) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    insert_log_tx(pool.user_id(), &mut tx, log).await?;
    tx.commit().await?;
    Ok(())
}
async fn insert_log_tx(
    user_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
    log: &MultiplierMonitorLog,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO multiplier_monitor_logs(user_id,id,run_id,config_id,config_name,account_id,account_name,started_at,finished_at,duration_ms,result,old_multiplier,remote_multiplier,error_code,error_message,http_status) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)").bind(user_id).bind(&log.id).bind(&log.run_id).bind(&log.config_id).bind(&log.config_name).bind(&log.account_id).bind(&log.account_name).bind(&log.started_at).bind(&log.finished_at).bind(log.duration_ms).bind(&log.result).bind(log.old_multiplier).bind(log.remote_multiplier).bind(&log.error_code).bind(&log.error_message).bind(log.http_status).execute(&mut **tx).await?;
    Ok(())
}
pub async fn list_logs(
    pool: &UserDb,
    id: &str,
    offset: i64,
    limit: i64,
    result: Option<&str>,
) -> Result<(Vec<MultiplierMonitorLog>, i64), AppError> {
    let (sql, count) = if result.is_some() {
        ("SELECT * FROM multiplier_monitor_logs WHERE user_id=? AND (account_id=? OR config_id=?) AND result=? ORDER BY started_at DESC,id DESC LIMIT ? OFFSET ?","SELECT COUNT(*) FROM multiplier_monitor_logs WHERE user_id=? AND (account_id=? OR config_id=?) AND result=?")
    } else {
        ("SELECT * FROM multiplier_monitor_logs WHERE user_id=? AND (account_id=? OR config_id=?) ORDER BY started_at DESC,id DESC LIMIT ? OFFSET ?","SELECT COUNT(*) FROM multiplier_monitor_logs WHERE user_id=? AND (account_id=? OR config_id=?)")
    };
    let mut q = sqlx::query_as::<_, MultiplierMonitorLog>(sql)
        .bind(pool.user_id())
        .bind(id)
        .bind(id);
    if let Some(v) = result {
        q = q.bind(v);
    }
    let items = q.bind(limit).bind(offset).fetch_all(pool.raw()).await?;
    let mut c = sqlx::query_scalar::<_, i64>(count)
        .bind(pool.user_id())
        .bind(id)
        .bind(id);
    if let Some(v) = result {
        c = c.bind(v);
    }
    Ok((items, c.fetch_one(pool.raw()).await?))
}
