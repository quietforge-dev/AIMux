use crate::{dao::user_dao, error::AppError, model::user::User};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

pub fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
pub fn new_token() -> String {
    format!(
        "aimux_{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
pub fn prefix(token: &str) -> String {
    token.chars().take(12).collect()
}
pub fn validate_username(username: &str) -> Result<(), AppError> {
    if !(1..=64).contains(&username.chars().count())
        || username
            .chars()
            .any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(AppError::BadRequest(
            "用户名须为 1 到 64 个非空白字符".into(),
        ));
    }
    Ok(())
}
pub async fn hash_password(password: &str) -> Result<String, AppError> {
    if !(8..=256).contains(&password.len()) {
        return Err(AppError::BadRequest("密码长度须为 8 到 256 字节".into()));
    }
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || {
        let random = uuid::Uuid::new_v4();
        let salt = SaltString::encode_b64(random.as_bytes())
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|v| v.to_string())
            .map_err(|e| AppError::Internal(e.to_string()))
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
pub async fn verify(password: &str, hash: &str) -> Result<bool, AppError> {
    if password.len() > 256 {
        return Ok(false);
    }
    let password = password.to_owned();
    let hash = hash.to_owned();
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).is_ok_and(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))
}
pub async fn login(
    pool: &SqlitePool,
    username: &str,
    password: &str,
) -> Result<(String, User), AppError> {
    let user = user_dao::by_username(pool, username).await?;
    // Use a real Argon2 hash even for an unknown name, avoiding a cheap account existence test.
    let dummy="$argon2id$v=19$m=19456,t=2,p=1$MDEyMzQ1Njc4OWFiY2RlZg$IEoN8Y8bFnr7+3zJZjHj+CeCPdP3GC+GwC70WbMFB2U";
    let valid = verify(
        password,
        user.as_ref().map_or(dummy, |u| u.password_hash.as_str()),
    )
    .await?;
    let user = user
        .filter(|u| u.enabled && u.initialized && valid)
        .ok_or(AppError::Unauthorized)?;
    let token = new_token();
    if !user_dao::session_create(pool, &token_hash(&token), &user.id, &user.password_hash).await? {
        return Err(AppError::Unauthorized);
    }
    Ok((token, user))
}
pub async fn create_user(
    pool: &SqlitePool,
    username: &str,
    password: &str,
) -> Result<(User, String), AppError> {
    let username = username.trim();
    validate_username(username)?;
    let hash = hash_password(password).await?;
    let token = new_token();
    let user =
        user_dao::create(pool, username, &hash, &token_hash(&token), &prefix(&token)).await?;
    Ok((user, token))
}
pub async fn reset_token(pool: &SqlitePool, user_id: &str) -> Result<String, AppError> {
    user_dao::get(pool, user_id).await?;
    let token = new_token();
    user_dao::set_token(pool, user_id, &token_hash(&token), &prefix(&token)).await?;
    Ok(token)
}
pub async fn change_password(
    pool: &SqlitePool,
    user_id: &str,
    old: &str,
    new: &str,
) -> Result<(), AppError> {
    let user = user_dao::get(pool, user_id).await?;
    if !verify(old, &user.password_hash).await? {
        return Err(AppError::BadRequest("当前密码不正确".into()));
    }
    let hash = hash_password(new).await?;
    user_dao::set_password(pool, user_id, &hash).await
}

pub async fn list_users(
    pool: &SqlitePool,
    offset: i64,
    limit: i64,
) -> Result<(Vec<User>, i64), AppError> {
    user_dao::list(pool, offset, limit).await
}
pub async fn set_user_enabled(pool: &SqlitePool, id: &str, enabled: bool) -> Result<(), AppError> {
    user_dao::set_enabled(pool, id, enabled).await
}
pub async fn reset_password(pool: &SqlitePool, id: &str, password: &str) -> Result<(), AppError> {
    user_dao::get(pool, id).await?;
    let hash = hash_password(password).await?;
    user_dao::set_password(pool, id, &hash).await
}
pub async fn logout(pool: &SqlitePool, session_hash: &str) -> Result<(), AppError> {
    user_dao::session_delete(pool, session_hash).await
}
