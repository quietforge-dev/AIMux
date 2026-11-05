use crate::{config::Settings, error::AppError, model::user::User, utils::time::utc_now_string};
use sqlx::SqlitePool;

pub async fn get(pool: &SqlitePool, id: &str) -> Result<User, AppError> {
    sqlx::query_as("SELECT * FROM users WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("用户不存在".into()))
}
pub async fn by_username(pool: &SqlitePool, username: &str) -> Result<Option<User>, AppError> {
    Ok(sqlx::query_as("SELECT * FROM users WHERE username=?")
        .bind(username)
        .fetch_optional(pool)
        .await?)
}
pub async fn list(
    pool: &SqlitePool,
    offset: i64,
    limit: i64,
) -> Result<(Vec<User>, i64), AppError> {
    let count = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    let rows = sqlx::query_as("SELECT * FROM users ORDER BY created_at,id LIMIT ? OFFSET ?")
        .bind(limit.clamp(1, 200))
        .bind(offset.max(0))
        .fetch_all(pool)
        .await?;
    Ok((rows, count))
}
pub async fn enabled(pool: &SqlitePool) -> Result<Vec<User>, AppError> {
    Ok(
        sqlx::query_as("SELECT * FROM users WHERE enabled=1 AND initialized=1 ORDER BY id")
            .fetch_all(pool)
            .await?,
    )
}
pub async fn initialize_owner(
    pool: &SqlitePool,
    username: &str,
    password: &str,
    token: Option<(&str, &str)>,
    settings: &Settings,
) -> Result<(), AppError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let initialized: bool = sqlx::query_scalar("SELECT initialized FROM users WHERE id='local'")
        .fetch_one(&mut *tx)
        .await?;
    if !initialized {
        sqlx::query("UPDATE users SET username=?,password_hash=?,api_token_hash=?,api_token_prefix=?,api_token_updated_at=?,initialized=1,updated_at=? WHERE id='local'")
            .bind(username).bind(password).bind(token.map(|x|x.0)).bind(token.map_or("",|x|x.1))
            .bind(token.map(|_|utc_now_string())).bind(utc_now_string()).execute(&mut *tx).await?;
        super::settings_dao::save_tx(&mut tx, "local", settings).await?;
    }
    tx.commit().await?;
    Ok(())
}
pub async fn create(
    pool: &SqlitePool,
    username: &str,
    password_hash: &str,
    token_hash: &str,
    token_prefix: &str,
) -> Result<User, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = utc_now_string();
    sqlx::query("INSERT INTO users(id,username,password_hash,role,api_token_hash,api_token_prefix,api_token_updated_at,initialized,created_at,updated_at) VALUES(?,?,?,'user',?,?,?,1,?,?)")
        .bind(&id).bind(username).bind(password_hash).bind(token_hash).bind(token_prefix).bind(&now).bind(&now).bind(&now).execute(pool).await
        .map_err(|e| if e.as_database_error().is_some_and(|e|e.is_unique_violation()) { AppError::BadRequest("用户名已存在".into()) } else { e.into() })?;
    get(pool, &id).await
}
pub async fn set_enabled(pool: &SqlitePool, id: &str, enabled: bool) -> Result<(), AppError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let role: Option<String> = sqlx::query_scalar("SELECT role FROM users WHERE id=?")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    match role.as_deref() {
        None => return Err(AppError::NotFound("用户不存在".into())),
        Some("admin") if !enabled => return Err(AppError::BadRequest("不能禁用唯一管理员".into())),
        _ => {}
    }
    sqlx::query("UPDATE users SET enabled=?,updated_at=? WHERE id=?")
        .bind(enabled)
        .bind(utc_now_string())
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if !enabled {
        sqlx::query("DELETE FROM web_sessions WHERE user_id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}
pub async fn set_password(pool: &SqlitePool, id: &str, hash: &str) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE users SET password_hash=?,updated_at=? WHERE id=?")
        .bind(hash)
        .bind(utc_now_string())
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM web_sessions WHERE user_id=?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
pub async fn set_token(
    pool: &SqlitePool,
    id: &str,
    hash: &str,
    prefix: &str,
) -> Result<(), AppError> {
    sqlx::query("UPDATE users SET api_token_hash=?,api_token_prefix=?,api_token_updated_at=?,updated_at=? WHERE id=?")
        .bind(hash).bind(prefix).bind(utc_now_string()).bind(utc_now_string()).bind(id).execute(pool).await?;
    Ok(())
}
pub async fn by_token(pool: &SqlitePool, hash: &str) -> Result<Option<User>, AppError> {
    Ok(
        sqlx::query_as(
            "SELECT * FROM users WHERE api_token_hash=? AND enabled=1 AND initialized=1",
        )
        .bind(hash)
        .fetch_optional(pool)
        .await?,
    )
}
pub async fn by_session(pool: &SqlitePool, hash: &str) -> Result<Option<User>, AppError> {
    Ok(sqlx::query_as("SELECT u.* FROM users u JOIN web_sessions s ON s.user_id=u.id WHERE s.token_hash=? AND s.expires_at>? AND u.enabled=1 AND u.initialized=1")
        .bind(hash).bind(chrono::Utc::now().timestamp()).fetch_optional(pool).await?)
}
pub async fn session_create(
    pool: &SqlitePool,
    hash: &str,
    id: &str,
    password_hash: &str,
) -> Result<bool, AppError> {
    let now = chrono::Utc::now().timestamp();
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM web_sessions WHERE expires_at<=?")
        .bind(now)
        .execute(&mut *tx)
        .await?;
    let r=sqlx::query("INSERT INTO web_sessions(token_hash,user_id,expires_at,created_at) SELECT ?,id,?,? FROM users WHERE id=? AND enabled=1 AND password_hash=?")
        .bind(hash).bind(now+7*86400).bind(utc_now_string()).bind(id).bind(password_hash).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(r.rows_affected() == 1)
}
pub async fn session_delete(pool: &SqlitePool, hash: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM web_sessions WHERE token_hash=?")
        .bind(hash)
        .execute(pool)
        .await?;
    Ok(())
}

/// A desktop database can acquire Web credentials once, without re-importing user settings.
pub async fn initialize_web_credentials(
    pool: &SqlitePool,
    username: &str,
    hash: &str,
) -> Result<(), AppError> {
    sqlx::query("UPDATE users SET username=?,password_hash=?,updated_at=? WHERE id='local' AND password_hash=''")
        .bind(username).bind(hash).bind(utc_now_string()).execute(pool).await?;
    Ok(())
}

pub async fn ids(pool: &SqlitePool) -> Result<Vec<String>, AppError> {
    Ok(sqlx::query_scalar("SELECT id FROM users ORDER BY id")
        .fetch_all(pool)
        .await?)
}
