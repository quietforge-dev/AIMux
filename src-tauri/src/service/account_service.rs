use crate::{
    dao::account_dao, error::AppError, model::account::Account, schema::account_schema::AccountView,
};
use sqlx::SqlitePool;

pub async fn to_view(pool: &SqlitePool, id: &str) -> Result<AccountView, AppError> {
    account_dao::get(pool, id)
        .await?
        .map(account_dao::to_view)
        .ok_or_else(|| AppError::NotFound("账号不存在".into()))
}
pub async fn record_failure(
    pool: &SqlitePool,
    id: &str,
    code: Option<&str>,
    message: Option<&str>,
) -> Result<(), AppError> {
    account_dao::adjust(pool, id, false, "gateway", code, message).await
}
pub async fn record_success(pool: &SqlitePool, id: &str) -> Result<(), AppError> {
    account_dao::adjust(pool, id, true, "request", None, None).await
}
pub async fn record_monitor(
    pool: &SqlitePool,
    id: &str,
    ok: bool,
    code: Option<&str>,
    message: Option<&str>,
) -> Result<(), AppError> {
    account_dao::adjust(pool, id, ok, "monitor", code, message).await
}
pub fn mapping(account: &Account, model: Option<&str>) -> Option<String> {
    let Some(requested) = model else { return None };
    let Some(raw) = account.model_mappings.as_deref() else {
        return Some(requested.into());
    };
    serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(raw)
        .ok()
        .and_then(|map| {
            map.get(requested)
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        })
        .or_else(|| Some(requested.into()))
}
pub fn supported_models(account: &Account) -> Vec<String> {
    account
        .supported_models
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default()
}
