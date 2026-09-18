use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    error::AppError,
    model::catalog_model::CatalogModel,
    schema::model_schema::{ModelCreate, ModelUpdate, ModelView},
    utils::time::utc_now_string,
};

pub async fn list(
    pool: &SqlitePool,
    kind: Option<&str>,
    provider: Option<&str>,
) -> Result<Vec<CatalogModel>, AppError> {
    let mut sql = String::from("SELECT * FROM models WHERE 1=1");
    if kind.is_some() {
        sql.push_str(" AND type=?");
    }
    if provider.is_some() {
        sql.push_str(" AND provider=?");
    }
    sql.push_str(" ORDER BY type, provider, is_default DESC, lower(name)");

    let mut query = sqlx::query_as::<_, CatalogModel>(&sql);
    if let Some(value) = kind {
        query = query.bind(value);
    }
    if let Some(value) = provider {
        query = query.bind(value);
    }
    Ok(query.fetch_all(pool).await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<CatalogModel>, AppError> {
    Ok(
        sqlx::query_as::<_, CatalogModel>("SELECT * FROM models WHERE id=?")
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn default_name(pool: &SqlitePool, kind: &str) -> Result<Option<String>, AppError> {
    Ok(
        sqlx::query_scalar("SELECT name FROM models WHERE type=? AND is_default=1 LIMIT 1")
            .bind(kind)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn insert_missing(
    pool: &SqlitePool,
    defaults: &[(&str, &str, &str)],
) -> Result<(), AppError> {
    let now = utc_now_string();
    for (kind, provider, name) in defaults {
        sqlx::query("INSERT OR IGNORE INTO models(id,name,type,provider,is_default,created_at,updated_at) VALUES(?,?,?,?,0,?,?)")
            .bind(Uuid::new_v4().to_string())
            .bind(name)
            .bind(kind)
            .bind(provider)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub async fn create(pool: &SqlitePool, payload: ModelCreate) -> Result<CatalogModel, AppError> {
    let name = payload.name.trim().to_owned();
    let kind = payload.model_type.trim().to_owned();
    let provider = payload.provider.trim().to_owned();
    validate(&name, &kind, &provider)?;
    if sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM models WHERE type=? AND name=?")
        .bind(&kind)
        .bind(&name)
        .fetch_one(pool)
        .await?
        > 0
    {
        return Err(AppError::BadRequest("该类型下的模型名称已存在".into()));
    }

    let id = Uuid::new_v4().to_string();
    let now = utc_now_string();
    sqlx::query("INSERT INTO models(id,name,type,provider,is_default,created_at,updated_at) VALUES(?,?,?,?,0,?,?)")
        .bind(&id)
        .bind(name)
        .bind(kind)
        .bind(provider)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;
    get(pool, &id)
        .await?
        .ok_or_else(|| AppError::Internal("创建模型后读取失败".into()))
}

pub async fn update(
    pool: &SqlitePool,
    current: CatalogModel,
    payload: ModelUpdate,
) -> Result<CatalogModel, AppError> {
    let current_id = current.id.clone();
    let name = payload.name.unwrap_or(current.name).trim().to_owned();
    let kind = payload
        .model_type
        .unwrap_or(current.r#type.clone())
        .trim()
        .to_owned();
    let provider = payload
        .provider
        .unwrap_or(current.provider)
        .trim()
        .to_owned();
    validate(&name, &kind, &provider)?;
    if sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM models WHERE type=? AND name=? AND id<>?")
        .bind(&kind)
        .bind(&name)
        .bind(&current_id)
        .fetch_one(pool)
        .await?
        > 0
    {
        return Err(AppError::BadRequest("该类型下的模型名称已存在".into()));
    }

    let is_default = if kind != current.r#type {
        0
    } else {
        current.is_default
    };
    sqlx::query("UPDATE models SET name=?,type=?,provider=?,is_default=?,updated_at=? WHERE id=?")
        .bind(name)
        .bind(kind)
        .bind(provider)
        .bind(is_default)
        .bind(utc_now_string())
        .bind(&current_id)
        .execute(pool)
        .await?;
    get(pool, &current_id)
        .await?
        .ok_or_else(|| AppError::Internal("更新模型后读取失败".into()))
}

pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM models WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?;
    ensure_defaults(pool).await
}

pub async fn set_default(
    pool: &SqlitePool,
    current: CatalogModel,
) -> Result<CatalogModel, AppError> {
    let mut transaction = pool.begin().await?;
    sqlx::query("UPDATE models SET is_default=0,updated_at=? WHERE type=?")
        .bind(utc_now_string())
        .bind(&current.r#type)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("UPDATE models SET is_default=1,updated_at=? WHERE id=?")
        .bind(utc_now_string())
        .bind(&current.id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    get(pool, &current.id)
        .await?
        .ok_or_else(|| AppError::Internal("设置默认模型后读取失败".into()))
}

pub async fn ensure_defaults(pool: &SqlitePool) -> Result<(), AppError> {
    for kind in ["openai", "anthropic"] {
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM models WHERE type=? AND is_default=1",
        )
        .bind(kind)
        .fetch_one(pool)
        .await?;
        if exists == 0 {
            if let Some(item) = sqlx::query_as::<_, CatalogModel>(
                "SELECT * FROM models WHERE type=? ORDER BY lower(name),id LIMIT 1",
            )
            .bind(kind)
            .fetch_optional(pool)
            .await?
            {
                sqlx::query("UPDATE models SET is_default=1,updated_at=? WHERE id=?")
                    .bind(utc_now_string())
                    .bind(item.id)
                    .execute(pool)
                    .await?;
            }
        }
    }
    Ok(())
}

pub fn to_view(model: CatalogModel) -> ModelView {
    ModelView {
        id: model.id,
        name: model.name,
        model_type: model.r#type,
        provider: model.provider,
        is_default: model.is_default,
        created_at: model.created_at,
        updated_at: model.updated_at,
    }
}

fn validate(name: &str, kind: &str, provider: &str) -> Result<(), AppError> {
    if name.is_empty() {
        return Err(AppError::BadRequest("模型名称不能为空".into()));
    }
    if !["openai", "anthropic"].contains(&kind) {
        return Err(AppError::BadRequest("协议类型不支持".into()));
    }
    if provider.is_empty() {
        return Err(AppError::BadRequest("模型供应商不能为空".into()));
    }
    Ok(())
}
