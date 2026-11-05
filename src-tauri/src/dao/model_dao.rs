use crate::database::UserDb;
use uuid::Uuid;

use crate::{
    error::AppError,
    model::catalog_model::CatalogModel,
    schema::model_schema::{ModelCreate, ModelUpdate, ModelView},
    utils::time::utc_now_string,
};

pub async fn list(
    pool: &UserDb,
    kind: Option<&str>,
    provider: Option<&str>,
) -> Result<Vec<CatalogModel>, AppError> {
    let mut sql = String::from("SELECT * FROM models WHERE user_id=?");
    if kind.is_some() {
        sql.push_str(" AND type=?");
    }
    if provider.is_some() {
        sql.push_str(" AND provider=?");
    }
    sql.push_str(" ORDER BY type, provider, is_default DESC, lower(name)");

    let mut query = sqlx::query_as::<_, CatalogModel>(&sql).bind(pool.user_id());
    if let Some(value) = kind {
        query = query.bind(value);
    }
    if let Some(value) = provider {
        query = query.bind(value);
    }
    Ok(query.fetch_all(pool.raw()).await?)
}

pub async fn get(pool: &UserDb, id: &str) -> Result<Option<CatalogModel>, AppError> {
    Ok(
        sqlx::query_as::<_, CatalogModel>("SELECT * FROM models WHERE user_id=?1 AND id=?2")
            .bind(pool.user_id())
            .bind(id)
            .fetch_optional(pool.raw())
            .await?,
    )
}

pub async fn default_name(pool: &UserDb, kind: &str) -> Result<Option<String>, AppError> {
    Ok(sqlx::query_scalar(
        "SELECT name FROM models WHERE user_id=?1 AND type=?2 AND is_default=1 LIMIT 1",
    )
    .bind(pool.user_id())
    .bind(kind)
    .fetch_optional(pool.raw())
    .await?)
}

pub async fn provider_for_model(
    pool: &UserDb,
    kind: &str,
    name: &str,
) -> Result<Option<String>, AppError> {
    Ok(
        sqlx::query_scalar("SELECT provider FROM models WHERE user_id=?1 AND type=?2 AND name=?3")
            .bind(pool.user_id())
            .bind(kind)
            .bind(name)
            .fetch_optional(pool.raw())
            .await?,
    )
}

pub async fn provider_exists(pool: &UserDb, provider: &str) -> Result<bool, AppError> {
    Ok(
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM models WHERE user_id=?1 AND provider=?2)")
            .bind(pool.user_id())
            .bind(provider)
            .fetch_one(pool.raw())
            .await?,
    )
}

pub async fn insert_missing(
    pool: &UserDb,
    defaults: &[(&str, &str, &str)],
) -> Result<(), AppError> {
    let now = utc_now_string();
    for (kind, provider, name) in defaults {
        sqlx::query("INSERT OR IGNORE INTO models(user_id,id,name,type,provider,is_default,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,0,?6,?7)").bind(pool.user_id())
            .bind(Uuid::new_v4().to_string())
            .bind(name)
            .bind(kind)
            .bind(provider)
            .bind(&now)
            .bind(&now)
            .execute(pool.raw())
            .await?;
    }
    Ok(())
}

pub async fn create(pool: &UserDb, payload: ModelCreate) -> Result<CatalogModel, AppError> {
    let name = payload.name.trim().to_owned();
    let kind = payload.model_type.trim().to_owned();
    let provider = payload.provider.trim().to_owned();
    validate(&name, &kind, &provider)?;
    if sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM models WHERE user_id=?1 AND type=?2 AND name=?3",
    )
    .bind(pool.user_id())
    .bind(&kind)
    .bind(&name)
    .fetch_one(pool.raw())
    .await?
        > 0
    {
        return Err(AppError::BadRequest("该类型下的模型名称已存在".into()));
    }

    let id = Uuid::new_v4().to_string();
    let now = utc_now_string();
    sqlx::query("INSERT INTO models(user_id,id,name,type,provider,is_default,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,0,?6,?7)").bind(pool.user_id())
        .bind(&id)
        .bind(name)
        .bind(kind)
        .bind(provider)
        .bind(&now)
        .bind(&now)
        .execute(pool.raw())
        .await?;
    get(pool, &id)
        .await?
        .ok_or_else(|| AppError::Internal("创建模型后读取失败".into()))
}

pub async fn update(
    pool: &UserDb,
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
    if sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM models WHERE user_id=?1 AND type=?2 AND name=?3 AND id<>?4",
    )
    .bind(pool.user_id())
    .bind(&kind)
    .bind(&name)
    .bind(&current_id)
    .fetch_one(pool.raw())
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
    sqlx::query("UPDATE models SET name=?2,type=?3,provider=?4,is_default=?5,updated_at=?6 WHERE user_id=?1 AND id=?7").bind(pool.user_id())
        .bind(name)
        .bind(kind)
        .bind(provider)
        .bind(is_default)
        .bind(utc_now_string())
        .bind(&current_id)
        .execute(pool.raw())
        .await?;
    get(pool, &current_id)
        .await?
        .ok_or_else(|| AppError::Internal("更新模型后读取失败".into()))
}

pub async fn delete(pool: &UserDb, id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM models WHERE user_id=?1 AND id=?2")
        .bind(pool.user_id())
        .bind(id)
        .execute(pool.raw())
        .await?;
    ensure_defaults(pool).await
}

pub async fn set_default(pool: &UserDb, current: CatalogModel) -> Result<CatalogModel, AppError> {
    let mut transaction = pool.begin().await?;
    sqlx::query("UPDATE models SET is_default=0,updated_at=?2 WHERE user_id=?1 AND type=?3")
        .bind(pool.user_id())
        .bind(utc_now_string())
        .bind(&current.r#type)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("UPDATE models SET is_default=1,updated_at=?2 WHERE user_id=?1 AND id=?3")
        .bind(pool.user_id())
        .bind(utc_now_string())
        .bind(&current.id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    get(pool, &current.id)
        .await?
        .ok_or_else(|| AppError::Internal("设置默认模型后读取失败".into()))
}

pub async fn ensure_defaults(pool: &UserDb) -> Result<(), AppError> {
    for kind in ["openai", "anthropic"] {
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM models WHERE user_id=?1 AND type=?2 AND is_default=1",
        )
        .bind(pool.user_id())
        .bind(kind)
        .fetch_one(pool.raw())
        .await?;
        if exists == 0 {
            if let Some(item) = sqlx::query_as::<_, CatalogModel>(
                "SELECT * FROM models WHERE user_id=?1 AND type=?2 ORDER BY lower(name),id LIMIT 1",
            )
            .bind(pool.user_id())
            .bind(kind)
            .fetch_optional(pool.raw())
            .await?
            {
                sqlx::query(
                    "UPDATE models SET is_default=1,updated_at=?2 WHERE user_id=?1 AND id=?3",
                )
                .bind(pool.user_id())
                .bind(utc_now_string())
                .bind(item.id)
                .execute(pool.raw())
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
