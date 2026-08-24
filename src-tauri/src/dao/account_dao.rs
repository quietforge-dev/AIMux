use serde_json::Value;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    error::AppError,
    model::account::Account,
    schema::account_schema::{AccountCreate, AccountUpdate, AccountView},
    utils::time::utc_now_string,
};

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<Account>, AppError> {
    Ok(
        sqlx::query_as::<_, Account>("SELECT * FROM accounts WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn list(
    pool: &SqlitePool,
    offset: i64,
    limit: i64,
    account_type: Option<&str>,
    status: Option<&str>,
) -> Result<(Vec<Account>, i64), AppError> {
    let mut sql = String::from("SELECT * FROM accounts WHERE 1=1");
    let mut count = String::from("SELECT COUNT(*) FROM accounts WHERE 1=1");
    if account_type.is_some() {
        sql.push_str(" AND type = ?");
        count.push_str(" AND type = ?");
    }
    if status.is_some() {
        sql.push_str(" AND status = ?");
        count.push_str(" AND status = ?");
    }
    sql.push_str(" ORDER BY CASE WHEN status = 'active' THEN 0 ELSE 1 END, priority DESC, multiplier ASC, monitor_average_duration_ms IS NULL ASC, monitor_average_duration_ms ASC, lower(name), id LIMIT ? OFFSET ?");
    let mut query = sqlx::query_as::<_, Account>(&sql);
    if let Some(value) = account_type {
        query = query.bind(value);
    }
    if let Some(value) = status {
        query = query.bind(value);
    }
    let accounts = query.bind(limit).bind(offset).fetch_all(pool).await?;
    let mut total_query = sqlx::query_scalar::<_, i64>(&count);
    if let Some(value) = account_type {
        total_query = total_query.bind(value);
    }
    if let Some(value) = status {
        total_query = total_query.bind(value);
    }
    Ok((accounts, total_query.fetch_one(pool).await?))
}

pub async fn pick_one(
    pool: &SqlitePool,
    model: Option<&str>,
    account_type: &str,
) -> Result<Option<Account>, AppError> {
    let accounts = sqlx::query_as::<_, Account>("SELECT * FROM accounts WHERE status='active' AND type=? ORDER BY priority DESC, multiplier ASC, monitor_average_duration_ms IS NULL ASC, monitor_average_duration_ms ASC, lower(name), id").bind(account_type).fetch_all(pool).await?;
    let mut eligible: Vec<Account> = accounts
        .into_iter()
        .filter(|a| supported(a.supported_models.as_deref(), model))
        .collect();
    eligible.sort_by(|a, b| {
        let a_explicit = model
            .map(|m| has_model(a.supported_models.as_deref(), m))
            .unwrap_or(false);
        let b_explicit = model
            .map(|m| has_model(b.supported_models.as_deref(), m))
            .unwrap_or(false);
        b_explicit
            .cmp(&a_explicit)
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

pub async fn create(pool: &SqlitePool, payload: AccountCreate) -> Result<Account, AppError> {
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
    sqlx::query("INSERT INTO accounts (id,name,type,base_url,api_key_encrypted,status,priority,multiplier,test_default_model,model_mappings,supported_models,tags,notes,total_requests,total_tokens,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,0,0,?,?)")
        .bind(&id).bind(payload.name.trim()).bind(&payload.account_type).bind(payload.base_url.trim_end_matches('/')).bind(payload.api_key).bind(payload.status).bind(payload.priority).bind(payload.multiplier).bind(payload.test_default_model).bind(json_text(payload.model_mappings)).bind(json_opt_vec(payload.supported_models)).bind(json_opt_vec(payload.tags)).bind(payload.notes).bind(&now).bind(&now).execute(pool).await?;
    get(pool, &id)
        .await?
        .ok_or_else(|| AppError::Internal("创建账号后读取失败".into()))
}

pub async fn update(
    pool: &SqlitePool,
    current: Account,
    payload: AccountUpdate,
) -> Result<Account, AppError> {
    let current_id = current.id.clone();
    let name = payload.name.unwrap_or_else(|| current.name.clone());
    let account_type = payload
        .account_type
        .unwrap_or_else(|| current.r#type.clone());
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
    sqlx::query("UPDATE accounts SET name=?,type=?,base_url=?,api_key_encrypted=?,status=?,priority=?,multiplier=?,test_default_model=?,model_mappings=?,supported_models=?,tags=?,notes=?,updated_at=? WHERE id=?")
        .bind(name).bind(account_type).bind(base_url.trim_end_matches('/')).bind(api_key).bind(status).bind(priority).bind(multiplier).bind(test_default_model).bind(model_mappings).bind(supported_models).bind(tags).bind(notes).bind(now).bind(&current_id).execute(pool).await?;
    get(pool, &current_id)
        .await?
        .ok_or_else(|| AppError::Internal("更新账号后读取失败".into()))
}

pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM accounts WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
pub async fn save_priority(pool: &SqlitePool, id: &str, priority: i64) -> Result<(), AppError> {
    sqlx::query("UPDATE accounts SET priority=?, updated_at=? WHERE id=?")
        .bind(priority.clamp(0, 9))
        .bind(utc_now_string())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
pub async fn toggle_status(pool: &SqlitePool, id: &str) -> Result<Option<Account>, AppError> {
    sqlx::query("UPDATE accounts SET status=CASE status WHEN 'active' THEN 'disabled' ELSE 'active' END, updated_at=? WHERE id=?").bind(utc_now_string()).bind(id).execute(pool).await?;
    get(pool, id).await
}
pub async fn mark_used(pool: &SqlitePool, id: &str) -> Result<(), AppError> {
    let now = utc_now_string();
    sqlx::query("UPDATE accounts SET total_requests=total_requests+1,last_used_at=?,updated_at=? WHERE id=?").bind(&now).bind(&now).bind(id).execute(pool).await?;
    Ok(())
}
pub async fn add_total_tokens(pool: &SqlitePool, id: &str, tokens: i64) -> Result<(), AppError> {
    sqlx::query("UPDATE accounts SET total_tokens=total_tokens+? WHERE id=?")
        .bind(tokens)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
pub async fn adjust(
    pool: &SqlitePool,
    id: &str,
    success: bool,
    kind: &str,
    code: Option<&str>,
    message: Option<&str>,
) -> Result<(), AppError> {
    let delta = if success {
        if kind == "monitor" {
            "CASE WHEN priority < 8 THEN priority + 1 ELSE priority END"
        } else {
            "priority + 1"
        }
    } else {
        "priority - 1"
    };
    let safe = format!("UPDATE accounts SET priority=MAX(0,MIN(9,{})),last_error_code=?,last_error_message=?,updated_at=? WHERE id=?",delta);
    sqlx::query(&safe)
        .bind(code)
        .bind(message)
        .bind(utc_now_string())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
pub fn to_view(a: Account) -> AccountView {
    AccountView {
        id: a.id,
        name: a.name,
        account_type: a.r#type,
        base_url: a.base_url,
        api_key: a.api_key_encrypted,
        status: a.status,
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
    if !(0..=9).contains(&p) || !(0.01..=0.30).contains(&m) {
        return Err(AppError::BadRequest("优先级或倍率超出范围".into()));
    }
    Ok(())
}
