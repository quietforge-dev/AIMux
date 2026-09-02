use crate::{dao::model_dao, error::AppError, schema::model_schema::ModelView};
use sqlx::SqlitePool;

const DEFAULTS: [(&str, &str); 6] = [
    ("openai", "gpt-5.6-sol"),
    ("openai", "gpt-5.6-terra"),
    ("openai", concat!("gpt-5.6-", "luna")),
    ("anthropic", "claude-opus-5"),
    ("anthropic", "claude-sonnet-5"),
    ("anthropic", "claude-fable-5-1"),
];

pub async fn seed(pool: &SqlitePool) -> Result<(), AppError> {
    model_dao::insert_missing(pool, &DEFAULTS).await?;
    model_dao::ensure_defaults(pool).await
}

pub async fn list(pool: &SqlitePool, kind: Option<&str>) -> Result<Vec<ModelView>, AppError> {
    Ok(model_dao::list(pool, kind)
        .await?
        .into_iter()
        .map(model_dao::to_view)
        .collect())
}
