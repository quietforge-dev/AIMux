use crate::{config::Settings, error::AppError, utils::time::parse_hhmm};
use tokio::sync::RwLock;

pub fn validate(settings: &Settings) -> Result<(), String> {
    if settings.port == 0 {
        return Err("端口必须大于 0".into());
    }
    for (name, penalty) in [
        ("首字超时", settings.first_token_timeout_priority_penalty),
        ("上游超时", settings.upstream_timeout_priority_penalty),
        ("请求失败", settings.request_failure_priority_penalty),
        ("低缓存率", settings.low_cache_priority_penalty),
        ("连续首字慢", settings.slow_first_token_priority_penalty),
    ] {
        if !(1..=3).contains(&penalty) {
            return Err(format!("{name}优先级扣减数必须为 1 到 3"));
        }
    }
    if !(1..=1000).contains(&settings.low_cache_consecutive) {
        return Err("连续低缓存次数必须为 1 到 1000".into());
    }
    if !(1..=1000).contains(&settings.slow_first_token_consecutive) {
        return Err("连续首字慢次数必须为 1 到 1000".into());
    }
    if !(1..=86400).contains(&settings.slow_first_token_threshold_seconds) {
        return Err("首字慢阈值必须为 1 到 86400 秒".into());
    }
    if !(1..=100).contains(&settings.low_cache_threshold_percent) {
        return Err("缓存率阈值必须为 1 到 100%".into());
    }
    if !(2..=10).contains(&settings.monitoring_interval_minutes) {
        return Err("检测间隔必须为 2 到 10 分钟".into());
    }
    if !(10..=40).contains(&settings.monitoring_recent_count) {
        return Err("最近记录数必须为 10 到 40 次".into());
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
    let mut current = settings.write().await;
    next.monitoring_enabled = current.monitoring_enabled;
    next.monitoring_start_time = current.monitoring_start_time.clone();
    next.monitoring_end_time = current.monitoring_end_time.clone();
    next.monitoring_interval_minutes = current.monitoring_interval_minutes;
    next.monitoring_recent_count = current.monitoring_recent_count;
    validate(&next).map_err(AppError::BadRequest)?;
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
    monitoring_interval_minutes: Option<u64>,
    monitoring_recent_count: Option<i64>,
) -> Result<Settings, AppError> {
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
    if let Some(value) = monitoring_interval_minutes {
        next.monitoring_interval_minutes = value;
    }
    if let Some(value) = monitoring_recent_count {
        next.monitoring_recent_count = value;
    }
    validate(&next).map_err(AppError::BadRequest)?;
    next.save()
        .map_err(|error| AppError::Internal(format!("保存账号监控设置失败：{error}")))?;
    *current = next.clone();
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_penalties_accept_only_one_to_three() {
        for value in [0, 1, 2, 3, 4] {
            for field in 0..5 {
                let mut settings = Settings::default();
                match field {
                    0 => settings.first_token_timeout_priority_penalty = value,
                    1 => settings.upstream_timeout_priority_penalty = value,
                    2 => settings.request_failure_priority_penalty = value,
                    3 => settings.low_cache_priority_penalty = value,
                    _ => settings.slow_first_token_priority_penalty = value,
                }
                assert_eq!(validate(&settings).is_ok(), (1..=3).contains(&value));
            }
        }
    }

    #[test]
    fn validates_low_cache_bounds() {
        for value in [0, 1, 1000, 1001] {
            let settings = Settings {
                low_cache_consecutive: value,
                ..Settings::default()
            };
            assert_eq!(validate(&settings).is_ok(), (1..=1000).contains(&value));
        }
        for value in [0, 1, 85, 100, 101] {
            let settings = Settings {
                low_cache_threshold_percent: value,
                ..Settings::default()
            };
            assert_eq!(validate(&settings).is_ok(), (1..=100).contains(&value));
        }
    }

    #[test]
    fn validates_slow_first_token_bounds() {
        for value in [0, 1, 1000, 1001] {
            let settings = Settings {
                slow_first_token_consecutive: value,
                ..Settings::default()
            };
            assert_eq!(validate(&settings).is_ok(), (1..=1000).contains(&value));
        }
        for value in [0, 1, 86400, 86401] {
            let settings = Settings {
                slow_first_token_threshold_seconds: value,
                ..Settings::default()
            };
            assert_eq!(validate(&settings).is_ok(), (1..=86400).contains(&value));
        }
    }
}
