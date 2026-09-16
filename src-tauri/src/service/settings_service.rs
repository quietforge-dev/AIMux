use crate::{config::Settings, error::AppError, utils::time::parse_hhmm};
use tokio::sync::RwLock;

pub fn validate(settings: &Settings) -> Result<(), String> {
    if settings.port == 0 {
        return Err("端口必须大于 0".into());
    }
    validate_monitoring_window(
        settings.monitoring_start_time.as_deref(),
        settings.monitoring_end_time.as_deref(),
    )
}

pub fn validate_monitoring_window(start: Option<&str>, end: Option<&str>) -> Result<(), String> {
    match (start, end) {
        (None, None) => Ok(()),
        (Some(start), Some(end)) => {
            let start = parse_hhmm(start).ok_or("监控开始时间格式必须为 HH:mm")?;
            let end = parse_hhmm(end).ok_or("监控结束时间格式必须为 HH:mm")?;
            if start == end {
                return Err("监控开始时间与结束时间不能相同".into());
            }
            Ok(())
        }
        (Some(_), None) | (None, Some(_)) => Err("监控时间范围必须同时设置开始和结束时间".into()),
    }
}

pub async fn update(settings: &RwLock<Settings>, mut next: Settings) -> Result<Settings, AppError> {
    validate(&next).map_err(AppError::BadRequest)?;
    let mut current = settings.write().await;
    next.monitoring_enabled = current.monitoring_enabled;
    next.monitoring_start_time = current.monitoring_start_time.clone();
    next.monitoring_end_time = current.monitoring_end_time.clone();
    next.save()
        .map_err(|error| AppError::Internal(format!("保存设置失败：{error}")))?;
    *current = next.clone();
    Ok(next)
}

pub async fn update_monitoring(
    settings: &RwLock<Settings>,
    monitoring_enabled: bool,
    monitoring_start_time: Option<String>,
    monitoring_end_time: Option<String>,
) -> Result<(bool, Option<String>, Option<String>), AppError> {
    validate_monitoring_window(
        monitoring_start_time.as_deref(),
        monitoring_end_time.as_deref(),
    )
    .map_err(AppError::BadRequest)?;
    let mut current = settings.write().await;
    let mut next = current.clone();
    next.monitoring_enabled = monitoring_enabled;
    next.monitoring_start_time = monitoring_start_time;
    next.monitoring_end_time = monitoring_end_time;
    next.save()
        .map_err(|error| AppError::Internal(format!("保存账号监控设置失败：{error}")))?;
    *current = next;
    Ok((
        monitoring_enabled,
        current.monitoring_start_time.clone(),
        current.monitoring_end_time.clone(),
    ))
}
