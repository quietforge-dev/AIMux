use std::{path::PathBuf, sync::Arc};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub upstream_timeout_seconds: u64,
    pub first_token_timeout_seconds: u64,
    pub request_retry_attempts: u32,
    #[serde(default = "default_priority_penalty")]
    pub first_token_timeout_priority_penalty: u8,
    #[serde(default = "default_priority_penalty")]
    pub upstream_timeout_priority_penalty: u8,
    #[serde(default = "default_request_failure_priority_penalty")]
    pub request_failure_priority_penalty: u8,
    #[serde(default)]
    pub low_cache_priority_enabled: bool,
    #[serde(default = "default_low_cache_consecutive")]
    pub low_cache_consecutive: u32,
    #[serde(default = "default_low_cache_threshold")]
    pub low_cache_threshold_percent: u8,
    #[serde(default = "default_low_cache_penalty")]
    pub low_cache_priority_penalty: u8,
    #[serde(default)]
    pub slow_first_token_enabled: bool,
    #[serde(default = "default_low_cache_consecutive")]
    pub slow_first_token_consecutive: u32,
    #[serde(default = "default_slow_first_token_seconds")]
    pub slow_first_token_threshold_seconds: u64,
    #[serde(default = "default_request_failure_priority_penalty")]
    pub slow_first_token_priority_penalty: u8,
    pub upstream_proxy_enabled: bool,
    pub upstream_proxy_url: String,
    pub monitoring_enabled: bool,
    #[serde(default = "default_monitoring_interval")]
    pub monitoring_interval_minutes: u64,
    #[serde(default = "default_monitoring_recent_count")]
    pub monitoring_recent_count: i64,
    #[serde(default)]
    pub monitoring_start_time: Option<String>,
    #[serde(default)]
    pub monitoring_end_time: Option<String>,
    pub local_token: String,
    pub launch_at_login: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 7789,
            upstream_timeout_seconds: 300,
            first_token_timeout_seconds: 60,
            request_retry_attempts: 10,
            first_token_timeout_priority_penalty: default_priority_penalty(),
            upstream_timeout_priority_penalty: default_priority_penalty(),
            request_failure_priority_penalty: default_request_failure_priority_penalty(),
            low_cache_priority_enabled: false,
            low_cache_consecutive: default_low_cache_consecutive(),
            low_cache_threshold_percent: default_low_cache_threshold(),
            low_cache_priority_penalty: default_low_cache_penalty(),
            slow_first_token_enabled: false,
            slow_first_token_consecutive: default_low_cache_consecutive(),
            slow_first_token_threshold_seconds: default_slow_first_token_seconds(),
            slow_first_token_priority_penalty: default_request_failure_priority_penalty(),
            upstream_proxy_enabled: false,
            upstream_proxy_url: "http://127.0.0.1:7890".into(),
            monitoring_enabled: true,
            monitoring_interval_minutes: default_monitoring_interval(),
            monitoring_recent_count: default_monitoring_recent_count(),
            monitoring_start_time: None,
            monitoring_end_time: None,
            local_token: String::new(),
            launch_at_login: false,
        }
    }
}

fn default_monitoring_interval() -> u64 {
    2
}
fn default_priority_penalty() -> u8 {
    1
}
fn default_low_cache_consecutive() -> u32 {
    5
}
fn default_low_cache_threshold() -> u8 {
    85
}
fn default_low_cache_penalty() -> u8 {
    3
}
fn default_slow_first_token_seconds() -> u64 {
    10
}
fn default_request_failure_priority_penalty() -> u8 {
    2
}
fn default_monitoring_recent_count() -> i64 {
    30
}

impl Settings {
    pub fn config_path() -> PathBuf {
        ProjectDirs::from("dev", "quietforge", "AIMux")
            .map(|d| d.config_dir().join("settings.json"))
            .unwrap_or_else(|| PathBuf::from("settings.json"))
    }
    pub fn data_dir() -> PathBuf {
        ProjectDirs::from("dev", "quietforge", "AIMux")
            .map(|d| d.data_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }
    pub fn database_path(&self) -> PathBuf {
        Self::data_dir().join("aimux.db")
    }
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path();
        let mut settings = if !path.exists() {
            let settings = Self::default();
            settings.save()?;
            settings
        } else {
            let raw = std::fs::read_to_string(path)?;
            serde_json::from_str(&raw).unwrap_or_default()
        };
        settings.apply_environment_overrides();
        Ok(settings)
    }
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    fn apply_environment_overrides(&mut self) {
        if let Ok(port) = std::env::var("AIMUX_PORT") {
            if let Ok(port) = port.parse::<u16>() {
                self.port = port;
            }
        }
        if let Ok(enabled) = std::env::var("AIMUX_MONITORING_ENABLED") {
            let normalized = enabled.trim().to_ascii_lowercase();
            if !normalized.is_empty() {
                self.monitoring_enabled = matches!(normalized.as_str(), "1" | "true");
            }
        }
    }
}

pub type SharedSettings = Arc<RwLock<Settings>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_settings_keep_existing_values_and_default_priority_penalties() {
        let original = Settings {
            port: 7790,
            request_retry_attempts: 7,
            ..Settings::default()
        };
        let mut value = serde_json::to_value(&original).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("first_token_timeout_priority_penalty");
        object.remove("upstream_timeout_priority_penalty");
        object.remove("request_failure_priority_penalty");
        object.remove("low_cache_priority_enabled");
        object.remove("low_cache_consecutive");
        object.remove("low_cache_threshold_percent");
        object.remove("low_cache_priority_penalty");
        object.remove("slow_first_token_enabled");
        object.remove("slow_first_token_consecutive");
        object.remove("slow_first_token_threshold_seconds");
        object.remove("slow_first_token_priority_penalty");
        let loaded: Settings = serde_json::from_value(value).unwrap();
        assert_eq!(loaded.port, 7790);
        assert_eq!(loaded.request_retry_attempts, 7);
        assert_eq!(loaded.first_token_timeout_priority_penalty, 1);
        assert_eq!(loaded.upstream_timeout_priority_penalty, 1);
        assert_eq!(loaded.request_failure_priority_penalty, 2);
        assert!(!loaded.low_cache_priority_enabled);
        assert_eq!(loaded.low_cache_consecutive, 5);
        assert_eq!(loaded.low_cache_threshold_percent, 85);
        assert_eq!(loaded.low_cache_priority_penalty, 3);
        assert!(!loaded.slow_first_token_enabled);
        assert_eq!(loaded.slow_first_token_consecutive, 5);
        assert_eq!(loaded.slow_first_token_threshold_seconds, 10);
        assert_eq!(loaded.slow_first_token_priority_penalty, 2);
    }
}
