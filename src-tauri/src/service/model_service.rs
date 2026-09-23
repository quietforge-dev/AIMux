use crate::{dao::model_dao, error::AppError, schema::model_schema::ModelView};
use sqlx::SqlitePool;

const DEFAULTS: [(&str, &str, &str); 10] = [
    ("openai", "openai", "gpt-5.6-sol"),
    ("openai", "openai", "gpt-5.6-terra"),
    ("openai", "openai", concat!("gpt-5.6-", "luna")),
    ("openai", "openai", "gpt-6-astra"),
    ("openai", "openai", "gpt-6-sol"),
    ("openai", "openai", "gpt-6-luna"),
    ("anthropic", "anthropic", "claude-opus-5"),
    ("anthropic", "anthropic", "claude-sonnet-5"),
    ("anthropic", "anthropic", "claude-fable-5-1"),
    ("anthropic", "anthropic", "claude-opus-5-5"),
];

pub async fn seed(pool: &SqlitePool) -> Result<(), AppError> {
    model_dao::insert_missing(pool, &DEFAULTS).await?;
    model_dao::ensure_defaults(pool).await
}

pub async fn list(pool: &SqlitePool, kind: Option<&str>) -> Result<Vec<ModelView>, AppError> {
    Ok(model_dao::list(pool, kind, None)
        .await?
        .into_iter()
        .map(model_dao::to_view)
        .collect())
}
