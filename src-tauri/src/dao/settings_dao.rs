use crate::{config::Settings, database::UserDb, error::AppError, utils::time::utc_now_string};
use sqlx::{Sqlite, Transaction};

fn is_user_key(key: &str) -> bool {
    !matches!(key, "host" | "port" | "local_token" | "launch_at_login")
        && serde_json::to_value(Settings::default()).is_ok_and(|v| v.get(key).is_some())
}

pub async fn save_tx(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: &str,
    settings: &Settings,
) -> Result<(), AppError> {
    let object = serde_json::to_value(settings).map_err(|e| AppError::Internal(e.to_string()))?;
    let now = utc_now_string();
    for (key, value) in object
        .as_object()
        .ok_or_else(|| AppError::Internal("设置格式错误".into()))?
    {
        if is_user_key(key) {
            sqlx::query("INSERT INTO user_settings(user_id,key,value_json,updated_at) VALUES(?,?,?,?) ON CONFLICT(user_id,key) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at")
                .bind(user_id).bind(key).bind(value.to_string()).bind(&now).execute(&mut **tx).await?;
        }
    }
    Ok(())
}

pub async fn save(db: &UserDb, settings: &Settings) -> Result<(), AppError> {
    let mut tx = db.begin().await?;
    save_tx(&mut tx, db.user_id(), settings).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn load(db: &UserDb, runtime: &Settings) -> Result<Settings, AppError> {
    let mut value =
        serde_json::to_value(Settings::default()).map_err(|e| AppError::Internal(e.to_string()))?;
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT key,value_json FROM user_settings WHERE user_id=?")
            .bind(db.user_id())
            .fetch_all(db.raw())
            .await?;
    for (key, raw) in rows {
        if is_user_key(&key) {
            value[&key] = serde_json::from_str(&raw)
                .map_err(|e| AppError::Internal(format!("用户设置 {key} 无效：{e}")))?;
        }
    }
    let mut settings: Settings = serde_json::from_value(value)
        .map_err(|e| AppError::Internal(format!("用户设置类型错误：{e}")))?;
    settings.host = runtime.host.clone();
    settings.port = runtime.port;
    settings.launch_at_login = runtime.launch_at_login;
    settings.local_token.clear();
    crate::service::settings_service::validate(&settings).map_err(AppError::Internal)?;
    Ok(settings)
}
