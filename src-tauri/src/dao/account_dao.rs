use crate::database::UserDb;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    error::AppError,
    model::account::Account,
    schema::account_schema::{AccountCreate, AccountUpdate, AccountView},
    utils::time::{unix_now_millis, utc_now_string},
};

pub async fn get(pool: &UserDb, id: &str) -> Result<Option<Account>, AppError> {
    Ok(
        sqlx::query_as::<_, Account>("SELECT * FROM accounts WHERE user_id=?1 AND id = ?2")
            .bind(pool.user_id())
            .bind(id)
            .fetch_optional(pool.raw())
            .await?,
    )
}

pub async fn list(
    pool: &UserDb,
    offset: i64,
    limit: i64,
    account_type: Option<&str>,
    status: Option<&str>,
    name: Option<&str>,
) -> Result<(Vec<Account>, i64), AppError> {
    expire_disabled(pool).await?;
    let mut sql = String::from("SELECT * FROM accounts WHERE user_id=?");
    let mut count = String::from("SELECT COUNT(*) FROM accounts WHERE user_id=?");
    if account_type.is_some() {
        sql.push_str(" AND type = ?");
        count.push_str(" AND type = ?");
    }
    if status.is_some() {
        sql.push_str(" AND status = ?");
        count.push_str(" AND status = ?");
    }
    if name.is_some() {
        sql.push_str(" AND lower(name) LIKE lower(?)");
        count.push_str(" AND lower(name) LIKE lower(?)");
    }
    sql.push_str(" ORDER BY CASE WHEN status = 'active' THEN 0 ELSE 1 END, priority DESC, multiplier ASC, monitor_average_duration_ms IS NULL ASC, monitor_average_duration_ms ASC, lower(name), id LIMIT ? OFFSET ?");
    let mut query = sqlx::query_as::<_, Account>(&sql).bind(pool.user_id());
    if let Some(value) = account_type {
        query = query.bind(value);
    }
    if let Some(value) = status {
        query = query.bind(value);
    }
    if let Some(value) = name {
        query = query.bind(format!("%{}%", value.trim()));
    }
    let accounts = query.bind(limit).bind(offset).fetch_all(pool.raw()).await?;
    let mut total_query = sqlx::query_scalar::<_, i64>(&count).bind(pool.user_id());
    if let Some(value) = account_type {
        total_query = total_query.bind(value);
    }
    if let Some(value) = status {
        total_query = total_query.bind(value);
    }
    if let Some(value) = name {
        total_query = total_query.bind(format!("%{}%", value.trim()));
    }
    Ok((accounts, total_query.fetch_one(pool.raw()).await?))
}

pub async fn pick_one(
    pool: &UserDb,
    model: Option<&str>,
    account_type: &str,
) -> Result<Option<Account>, AppError> {
    let provider = match model {
        Some(name) => super::model_dao::provider_for_model(pool, account_type, name).await?,
        None => None,
    };
    pick_account(pool, model, account_type, provider.as_deref()).await
}

/// Monitoring compares ordinary candidates without applying the user's routing override.
pub async fn pick_one_normal(
    pool: &UserDb,
    model: Option<&str>,
    account_type: &str,
) -> Result<Option<Account>, AppError> {
    pick_account(pool, model, account_type, None).await
}

async fn pick_account(
    pool: &UserDb,
    model: Option<&str>,
    account_type: &str,
    provider: Option<&str>,
) -> Result<Option<Account>, AppError> {
    expire_disabled(pool).await?;
    let accounts = sqlx::query_as::<_, Account>("SELECT * FROM accounts WHERE user_id=?1 AND status='active' AND type=?2 ORDER BY priority DESC, multiplier ASC, monitor_average_duration_ms IS NULL ASC, monitor_average_duration_ms ASC, lower(name), id").bind(pool.user_id()).bind(account_type).fetch_all(pool.raw()).await?;
    let mut eligible: Vec<Account> = accounts
        .into_iter()
        .filter(|a| supported(a.supported_models.as_deref(), model))
        .collect();
    eligible.sort_by(|a, b| {
        let is_super = |account: &Account| {
            account.super_priority
                && account.priority >= 5
                && provider.is_some()
                && account.provider.as_deref() == provider
        };
        let a_explicit = model
            .map(|m| has_model(a.supported_models.as_deref(), m))
            .unwrap_or(false);
        let b_explicit = model
            .map(|m| has_model(b.supported_models.as_deref(), m))
            .unwrap_or(false);
        is_super(b)
            .cmp(&is_super(a))
            .then_with(|| b_explicit.cmp(&a_explicit))
            .then_with(|| b.priority.cmp(&a.priority))
            .then_with(|| a.multiplier.total_cmp(&b.multiplier))
            .then_with(|| {
                a.monitor_average_duration_ms
                    .unwrap_or(i64::MAX)
                    .cmp(&b.monitor_average_duration_ms.unwrap_or(i64::MAX))
            })
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(eligible.into_iter().next())
}

fn has_model(raw: Option<&str>, model: &str) -> bool {
    raw.and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
        .map(|items| items.iter().any(|item| item == model))
        .unwrap_or(false)
}
fn supported(raw: Option<&str>, model: Option<&str>) -> bool {
    let Some(model) = model else { return true };
    let Some(raw) = raw else { return true };
    let Ok(models) = serde_json::from_str::<Vec<String>>(raw) else {
        return true;
    };
    models.is_empty() || models.iter().any(|item| item == model)
}

pub async fn create(pool: &UserDb, payload: AccountCreate) -> Result<Account, AppError> {
    validate(
        &payload.name,
        &payload.account_type,
        &payload.base_url,
        &payload.api_key,
        payload.priority,
        payload.multiplier,
    )?;
    let now = utc_now_string();
    let id = Uuid::new_v4().to_string();
    let provider = normalize_provider(payload.provider);
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    if payload.super_priority {
        clear_other_super_priorities(
            pool.user_id(),
            &mut tx,
            &id,
            &payload.account_type,
            provider.as_deref(),
        )
        .await?;
    }
    sqlx::query("INSERT INTO accounts (user_id,id,name,type,base_url,api_key_encrypted,status,priority,multiplier,test_default_model,model_mappings,supported_models,tags,notes,total_requests,total_tokens,created_at,updated_at,provider,super_priority) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,0,0,?15,?16,?17,?18)").bind(pool.user_id())
        .bind(&id).bind(payload.name.trim()).bind(&payload.account_type).bind(payload.base_url.trim_end_matches('/')).bind(payload.api_key).bind(payload.status).bind(payload.priority).bind(payload.multiplier).bind(payload.test_default_model).bind(json_text(payload.model_mappings)).bind(json_opt_vec(payload.supported_models)).bind(json_opt_vec(payload.tags)).bind(payload.notes).bind(&now).bind(&now).bind(provider).bind(payload.super_priority).execute(&mut *tx).await?;
    tx.commit().await?;
    get(pool, &id)
        .await?
        .ok_or_else(|| AppError::Internal("创建账号后读取失败".into()))
}

pub async fn update(
    pool: &UserDb,
    current: Account,
    payload: AccountUpdate,
) -> Result<Account, AppError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    // Read after acquiring the write lock so a stale form cannot resurrect a replaced override.
    let current = sqlx::query_as::<_, Account>("SELECT * FROM accounts WHERE user_id=?1 AND id=?2")
        .bind(pool.user_id())
        .bind(&current.id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("账号不存在".into()))?;
    let current_id = current.id.clone();
    let name = payload.name.unwrap_or_else(|| current.name.clone());
    let account_type = payload
        .account_type
        .unwrap_or_else(|| current.r#type.clone());
    let provider = match payload.provider {
        Some(value) => normalize_provider(Some(value)),
        None => current.provider.clone(),
    };
    let super_priority = payload
        .super_priority
        .unwrap_or(current.super_priority && provider.is_some());
    let base_url = payload.base_url.unwrap_or_else(|| current.base_url.clone());
    let api_key = payload
        .api_key
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| current.api_key_encrypted.clone());
    let status = payload.status.unwrap_or_else(|| current.status.clone());
    let priority = payload.priority.unwrap_or(current.priority);
    let multiplier = payload.multiplier.unwrap_or(current.multiplier);
    let test_default_model = payload
        .test_default_model
        .or_else(|| current.test_default_model.clone());
    let model_mappings = match payload.model_mappings {
        Some(value) if value.as_object().is_some_and(|items| items.is_empty()) => None,
        Some(value) => json_text(Some(value)),
        None => current.model_mappings.clone(),
    };
    let supported_models = match payload.supported_models {
        Some(value) => json_opt_vec(Some(value)),
        None => current.supported_models.clone(),
    };
    let tags = match payload.tags {
        Some(value) => json_opt_vec(Some(value)),
        None => current.tags.clone(),
    };
    let notes = match payload.notes {
        Some(value) if value.trim().is_empty() => None,
        Some(value) => Some(value),
        None => current.notes.clone(),
    };
    validate(
        &name,
        &account_type,
        &base_url,
        &api_key,
        priority,
        multiplier,
    )?;
    let now = utc_now_string();
    if super_priority {
        clear_other_super_priorities(
            pool.user_id(),
            &mut tx,
            &current_id,
            &account_type,
            provider.as_deref(),
        )
        .await?;
    }
    sqlx::query("UPDATE accounts SET name=?2,type=?3,base_url=?4,api_key_encrypted=?5,status=?6,priority=?7,multiplier=?8,test_default_model=?9,model_mappings=?10,supported_models=?11,tags=?12,notes=?13,updated_at=?14,provider=?15,super_priority=?16 WHERE user_id=?1 AND id=?17").bind(pool.user_id())
        .bind(name).bind(account_type).bind(base_url.trim_end_matches('/')).bind(api_key).bind(status).bind(priority).bind(multiplier).bind(test_default_model).bind(model_mappings).bind(supported_models).bind(tags).bind(notes).bind(now).bind(provider).bind(super_priority).bind(&current_id).execute(&mut *tx).await?;
    tx.commit().await?;
    get(pool, &current_id)
        .await?
        .ok_or_else(|| AppError::Internal("更新账号后读取失败".into()))
}

pub async fn delete(pool: &UserDb, id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM multiplier_monitor_configs WHERE user_id=?1 AND account_id=?2")
        .bind(pool.user_id())
        .bind(id)
        .execute(pool.raw())
        .await?;
    sqlx::query("DELETE FROM accounts WHERE user_id=?1 AND id=?2")
        .bind(pool.user_id())
        .bind(id)
        .execute(pool.raw())
        .await?;
    Ok(())
}
pub async fn save_priority(pool: &UserDb, id: &str, priority: i64) -> Result<(), AppError> {
    sqlx::query("UPDATE accounts SET priority=?2, updated_at=?3 WHERE user_id=?1 AND id=?4")
        .bind(pool.user_id())
        .bind(priority.clamp(0, 9))
        .bind(utc_now_string())
        .bind(id)
        .execute(pool.raw())
        .await?;
    Ok(())
}
pub async fn toggle_status(pool: &UserDb, id: &str) -> Result<Option<Account>, AppError> {
    let current = get(pool, id).await?;
    let Some(current) = current else {
        return Ok(None);
    };
    if current.status == "active" {
        set_status(pool, id, "disabled", None).await
    } else {
        set_status(pool, id, "active", None).await
    }
}

pub async fn set_status(
    pool: &UserDb,
    id: &str,
    status: &str,
    duration_hours: Option<i64>,
) -> Result<Option<Account>, AppError> {
    let disabled_until = match status {
        "active" => None,
        "disabled" => match duration_hours {
            None => None,
            Some(hours) if matches!(hours, 1 | 3 | 12) => {
                Some(unix_now_millis() + hours * 60 * 60 * 1000)
            }
            Some(_) => {
                return Err(AppError::BadRequest(
                    "禁用时长只能是 1、3、12 小时或永久禁用".into(),
                ))
            }
        },
        _ => return Err(AppError::BadRequest("账号状态不支持".into())),
    };
    sqlx::query("UPDATE accounts SET status=?2, disabled_until=?3, updated_at=?4 WHERE user_id=?1 AND id=?5").bind(pool.user_id())
        .bind(status)
        .bind(disabled_until)
        .bind(utc_now_string())
        .bind(id)
        .execute(pool.raw())
        .await?;
    get(pool, id).await
}

pub async fn expire_disabled(pool: &UserDb) -> Result<u64, AppError> {
    let result = sqlx::query("UPDATE accounts SET status='active', disabled_until=NULL, updated_at=?2 WHERE user_id=?1 AND status='disabled' AND disabled_until IS NOT NULL AND disabled_until <= ?3").bind(pool.user_id())
        .bind(utc_now_string())
        .bind(unix_now_millis())
        .execute(pool.raw())
        .await?;
    Ok(result.rows_affected())
}
pub async fn mark_used(pool: &UserDb, id: &str) -> Result<(), AppError> {
    let now = utc_now_string();
    sqlx::query("UPDATE accounts SET total_requests=total_requests+1,last_used_at=?2,updated_at=?3 WHERE user_id=?1 AND id=?4").bind(pool.user_id()).bind(&now).bind(&now).bind(id).execute(pool.raw()).await?;
    Ok(())
}
pub async fn add_total_tokens(pool: &UserDb, id: &str, tokens: i64) -> Result<(), AppError> {
    sqlx::query("UPDATE accounts SET total_tokens=total_tokens+?2 WHERE user_id=?1 AND id=?3")
        .bind(pool.user_id())
        .bind(tokens)
        .bind(id)
        .execute(pool.raw())
        .await?;
    Ok(())
}
pub async fn adjust(
    pool: &UserDb,
    id: &str,
    success: bool,
    kind: &str,
    code: Option<&str>,
    message: Option<&str>,
) -> Result<(), AppError> {
    let delta = if success {
        1
    } else if kind == "gateway" {
        -2
    } else {
        -1
    };
    adjust_by(pool, id, delta, code, message).await
}
pub async fn adjust_by(
    pool: &UserDb,
    id: &str,
    delta: i64,
    code: Option<&str>,
    message: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query("UPDATE accounts SET priority=MAX(0,MIN(9,priority + ?2)),last_error_code=?3,last_error_message=?4,updated_at=?5 WHERE user_id=?1 AND id=?6").bind(pool.user_id())
        .bind(delta)
        .bind(code)
        .bind(message)
        .bind(utc_now_string())
        .bind(id)
        .execute(pool.raw())
        .await?;
    Ok(())
}
pub fn to_view(a: Account) -> AccountView {
    AccountView {
        id: a.id,
        name: a.name,
        account_type: a.r#type,
        provider: a.provider,
        super_priority: a.super_priority,
        base_url: a.base_url,
        api_key: a.api_key_encrypted,
        status: a.status,
        disabled_until: a.disabled_until,
        priority: a.priority,
        multiplier: a.multiplier,
        test_default_model: a.test_default_model,
        model_mappings: a
            .model_mappings
            .and_then(|s| serde_json::from_str::<Value>(&s).ok()),
        supported_models: a
            .supported_models
            .and_then(|s| serde_json::from_str(&s).ok()),
        tags: a.tags.and_then(|s| serde_json::from_str(&s).ok()),
        notes: a.notes,
        last_error_code: a.last_error_code,
        last_error_message: a.last_error_message,
        last_successful_test_model: a.last_successful_test_model,
        last_used_at: a.last_used_at,
        total_requests: a.total_requests,
        total_tokens: a.total_tokens,
        monitor_average_duration_ms: a.monitor_average_duration_ms,
        created_at: a.created_at,
        updated_at: a.updated_at,
    }
}

fn normalize_provider(provider: Option<String>) -> Option<String> {
    provider
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

async fn clear_other_super_priorities(
    user_id: &str,
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: &str,
    account_type: &str,
    provider: Option<&str>,
) -> Result<(), AppError> {
    let provider = provider
        .ok_or_else(|| AppError::BadRequest("请先指定账号供应商，再开启超级优先".into()))?;
    sqlx::query("UPDATE accounts SET super_priority=0,updated_at=?2 WHERE user_id=?1 AND type=?3 AND provider=?4 AND id<>?5 AND super_priority<>0").bind(user_id)
        .bind(utc_now_string()).bind(account_type).bind(provider).bind(id).execute(&mut **tx).await?;
    Ok(())
}

pub async fn set_super_priority(
    pool: &UserDb,
    id: &str,
    enabled: bool,
) -> Result<Account, AppError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let current = sqlx::query_as::<_, Account>("SELECT * FROM accounts WHERE user_id=?1 AND id=?2")
        .bind(pool.user_id())
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("账号不存在".into()))?;
    if enabled {
        clear_other_super_priorities(
            pool.user_id(),
            &mut tx,
            id,
            &current.r#type,
            current.provider.as_deref(),
        )
        .await?;
    }
    sqlx::query("UPDATE accounts SET super_priority=?2,updated_at=?3 WHERE user_id=?1 AND id=?4")
        .bind(pool.user_id())
        .bind(enabled)
        .bind(utc_now_string())
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    get(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("账号不存在".into()))
}
fn json_text(value: Option<Value>) -> Option<String> {
    value.and_then(|v| {
        if v.is_null() {
            None
        } else {
            Some(v.to_string())
        }
    })
}
fn json_opt_vec(value: Option<Vec<String>>) -> Option<String> {
    value
        .filter(|v| !v.is_empty())
        .and_then(|v| serde_json::to_string(&v).ok())
}
fn validate(name: &str, typ: &str, url: &str, key: &str, p: i64, m: f64) -> Result<(), AppError> {
    if name.trim().is_empty() || url.trim().is_empty() || key.trim().is_empty() {
        return Err(AppError::BadRequest(
            "名称、上游地址和 API 密钥不能为空".into(),
        ));
    }
    if !["openai", "anthropic"].contains(&typ) {
        return Err(AppError::BadRequest("协议类型不支持".into()));
    }
    if !(0..=9).contains(&p) || !(0.01..=0.99).contains(&m) {
        return Err(AppError::BadRequest("优先级或倍率超出范围".into()));
    }
    Ok(())
}

#[cfg(test)]
#[path = "account_dao_tests.rs"]
mod tests;
