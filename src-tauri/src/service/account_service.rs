use crate::database::UserDb;
use crate::{
    config::Settings,
    dao::account_dao,
    dao::model_dao,
    error::AppError,
    model::account::Account,
    schema::account_schema::{AccountCreate, AccountUpdate, AccountView},
};

async fn validate_provider(pool: &UserDb, provider: Option<&str>) -> Result<(), AppError> {
    let Some(provider) = provider.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    if [
        "openai",
        "anthropic",
        "xai",
        "google",
        "deepseek",
        "moonshot",
        "alibaba",
        "zhipu",
        "stepfun",
    ]
    .contains(&provider)
    {
        return Ok(());
    }
    if !model_dao::provider_exists(pool, provider).await? {
        return Err(AppError::BadRequest("供应商不在模型目录中".into()));
    }
    Ok(())
}

pub async fn create(pool: &UserDb, payload: AccountCreate) -> Result<Account, AppError> {
    validate_provider(pool, payload.provider.as_deref()).await?;
    account_dao::create(pool, payload).await
}

pub async fn update(
    pool: &UserDb,
    current: Account,
    payload: AccountUpdate,
) -> Result<Account, AppError> {
    if payload
        .provider
        .as_ref()
        .is_some_and(|value| Some(value.as_str()) != current.provider.as_deref())
    {
        validate_provider(pool, payload.provider.as_deref()).await?;
    }
    account_dao::update(pool, current, payload).await
}

pub async fn set_super_priority(
    pool: &UserDb,
    id: &str,
    enabled: bool,
) -> Result<Account, AppError> {
    account_dao::set_super_priority(pool, id, enabled).await
}

pub async fn to_view(pool: &UserDb, id: &str) -> Result<AccountView, AppError> {
    account_dao::get(pool, id)
        .await?
        .map(account_dao::to_view)
        .ok_or_else(|| AppError::NotFound("账号不存在".into()))
}
pub async fn record_failure(
    pool: &UserDb,
    id: &str,
    settings: &Settings,
    code: Option<&str>,
    message: Option<&str>,
) -> Result<(), AppError> {
    let penalty = failure_priority_penalty(settings, code);
    account_dao::adjust_by(pool, id, -i64::from(penalty), code, message).await
}
fn failure_priority_penalty(settings: &Settings, code: Option<&str>) -> u8 {
    if code == Some("first_token_timeout") {
        settings.first_token_timeout_priority_penalty
    } else if code == Some("upstream_timeout") {
        settings.upstream_timeout_priority_penalty
    } else {
        settings.request_failure_priority_penalty
    }
    .clamp(1, 3)
}
pub async fn record_success(pool: &UserDb, id: &str) -> Result<(), AppError> {
    account_dao::adjust(pool, id, true, "request", None, None).await
}
pub async fn record_monitor(
    pool: &UserDb,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_use_the_matching_timeout_or_request_policy() {
        let settings = Settings {
            first_token_timeout_priority_penalty: 3,
            upstream_timeout_priority_penalty: 2,
            request_failure_priority_penalty: 1,
            ..Settings::default()
        };
        assert_eq!(
            failure_priority_penalty(&settings, Some("first_token_timeout")),
            3
        );
        assert_eq!(
            failure_priority_penalty(&settings, Some("upstream_error")),
            1
        );
        assert_eq!(
            failure_priority_penalty(&settings, Some("upstream_timeout")),
            2
        );
        assert_eq!(
            failure_priority_penalty(&settings, Some("stream_read_error")),
            1
        );
    }

    #[tokio::test]
    async fn failure_deduction_is_applied_once_and_priority_stays_nonnegative() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let pool = UserDb::new(pool, "local");
        sqlx::query("CREATE TABLE accounts (user_id TEXT NOT NULL DEFAULT 'local', id TEXT PRIMARY KEY, priority INTEGER, last_error_code TEXT, last_error_message TEXT, updated_at TEXT)")
            .execute(pool.raw()).await.unwrap();
        sqlx::query("INSERT INTO accounts(id,priority) VALUES('test',9)")
            .execute(pool.raw())
            .await
            .unwrap();
        let settings = Settings::default();
        for (code, expected) in [
            ("upstream_error", 7_i64),
            ("upstream_timeout", 6),
            ("first_token_timeout", 5),
        ] {
            record_failure(&pool, "test", &settings, Some(code), Some("失败"))
                .await
                .unwrap();
            let priority: i64 = sqlx::query_scalar("SELECT priority FROM accounts WHERE id='test'")
                .fetch_one(pool.raw())
                .await
                .unwrap();
            assert_eq!(priority, expected);
        }
        sqlx::query("UPDATE accounts SET priority=1")
            .execute(pool.raw())
            .await
            .unwrap();
        record_failure(&pool, "test", &settings, Some("upstream_error"), None)
            .await
            .unwrap();
        let priority: i64 = sqlx::query_scalar("SELECT priority FROM accounts WHERE id='test'")
            .fetch_one(pool.raw())
            .await
            .unwrap();
        assert_eq!(priority, 0);
    }
}
