use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    error::AppError,
    model::catalog_model::CatalogModel,
    schema::model_schema::{ModelCreate, ModelUpdate, ModelView},
    utils::time::utc_now_string,
};

pub async fn list(pool: &SqlitePool, kind: Option<&str>) -> Result<Vec<CatalogModel>, AppError> {
    let mut q = String::from("SELECT * FROM models");
    if kind.is_some() {
        q.push_str(" WHERE type=?");
    }
    q.push_str(" ORDER BY type, is_default DESC, lower(name), id");
    let mut query = sqlx::query_as::<_, CatalogModel>(&q);
    if let Some(k) = kind {
        query = query.bind(k);
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
pub async fn insert_missing(pool: &SqlitePool, defaults: &[(&str, &str)]) -> Result<(), AppError> {
    let now = utc_now_string();
    for (kind, name) in defaults {
        sqlx::query("INSERT OR IGNORE INTO models(id,name,type,is_default,created_at,updated_at) VALUES(?,?,?,0,?,?)")
            .bind(Uuid::new_v4().to_string())
            .bind(name)
            .bind(kind)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;
    }
    Ok(())
}
pub async fn create(pool: &SqlitePool, p: ModelCreate) -> Result<CatalogModel, AppError> {
    if !["openai", "anthropic"].contains(&p.model_type.as_str()) {
        return Err(AppError::BadRequest("协议类型不支持".into()));
    }
    if sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM models WHERE type=? AND name=?")
        .bind(&p.model_type)
        .bind(p.name.trim())
        .fetch_one(pool)
        .await?
        > 0
    {
        return Err(AppError::BadRequest("该类型下的模型名称已存在".into()));
    }
    let id = Uuid::new_v4().to_string();
    let now = utc_now_string();
    sqlx::query(
        "INSERT INTO models(id,name,type,is_default,created_at,updated_at) VALUES(?,?,?,0,?,?)",
    )
    .bind(&id)
    .bind(p.name.trim())
    .bind(p.model_type)
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
    p: ModelUpdate,
) -> Result<CatalogModel, AppError> {
    let name = p.name.unwrap_or(current.name.clone());
    let kind = p.model_type.unwrap_or(current.r#type.clone());
    if sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM models WHERE type=? AND name=? AND id<>?")
        .bind(&kind)
        .bind(&name)
        .bind(&current.id)
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
    sqlx::query("UPDATE models SET name=?,type=?,is_default=?,updated_at=? WHERE id=?")
        .bind(name)
        .bind(kind)
        .bind(is_default)
        .bind(utc_now_string())
        .bind(&current.id)
        .execute(pool)
        .await?;
    get(pool, &current.id)
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
pub fn to_view(m: CatalogModel) -> ModelView {
    ModelView {
        id: m.id,
        name: m.name,
        model_type: m.r#type,
        is_default: m.is_default,
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}
