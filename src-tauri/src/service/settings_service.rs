use crate::{config::Settings, error::AppError};
use tokio::sync::RwLock;

pub fn validate(settings: &Settings) -> Result<(), String> {
    if settings.port == 0 {
        return Err("端口必须大于 0".into());
    }
    Ok(())
}

pub async fn update(settings: &RwLock<Settings>, mut next: Settings) -> Result<Settings, AppError> {
    validate(&next).map_err(AppError::BadRequest)?;
    let mut current = settings.write().await;
    next.monitoring_enabled = current.monitoring_enabled;
    next.save()
        .map_err(|error| AppError::Internal(format!("保存设置失败：{error}")))?;
    *current = next.clone();
    Ok(next)
}

pub async fn update_monitoring(
    settings: &RwLock<Settings>,
    monitoring_enabled: bool,
) -> Result<bool, AppError> {
    let mut current = settings.write().await;
    let mut next = current.clone();
    next.monitoring_enabled = monitoring_enabled;
    next.save()
        .map_err(|error| AppError::Internal(format!("保存账号监控设置失败：{error}")))?;
    *current = next;
    Ok(monitoring_enabled)
}
