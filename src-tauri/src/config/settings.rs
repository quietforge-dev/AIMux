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
    pub upstream_proxy_enabled: bool,
    pub upstream_proxy_url: String,
    pub monitoring_enabled: bool,
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
            upstream_proxy_enabled: false,
            upstream_proxy_url: "http://127.0.0.1:7890".into(),
            monitoring_enabled: true,
            monitoring_start_time: None,
            monitoring_end_time: None,
            local_token: String::new(),
            launch_at_login: false,
        }
    }
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
