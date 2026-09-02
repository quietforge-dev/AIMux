use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsPayload {
    pub host: String,
    pub port: u16,
    pub upstream_timeout_seconds: u64,
    pub first_token_timeout_seconds: u64,
    pub request_retry_attempts: u32,
    pub upstream_proxy_enabled: bool,
    pub upstream_proxy_url: String,
    pub monitoring_enabled: bool,
    pub local_token: String,
    pub launch_at_login: bool,
}

#[derive(Debug, Deserialize)]
pub struct MonitoringSettingsUpdate {
    pub monitoring_enabled: bool,
}
