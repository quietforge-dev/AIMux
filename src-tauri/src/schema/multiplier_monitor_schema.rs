use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct MultiplierMonitorCreate {
    pub name: String,
    pub url: String,
    pub token: String,
    pub account_ids: Vec<String>,
    #[serde(default = "default_multiplier_divisor")]
    pub multiplier_divisor: i64,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct MultiplierMonitorUpdate {
    pub name: String,
    pub url: String,
    pub token: Option<String>,
    pub account_ids: Vec<String>,
    pub multiplier_divisor: Option<i64>,
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

fn default_multiplier_divisor() -> i64 {
    1
}

#[derive(Debug, Clone, Serialize)]
pub struct MultiplierMonitorConfigView {
    pub id: String,
    pub name: String,
    pub url: String,
    pub account_ids: Vec<String>,
    pub multiplier_divisor: i64,
    pub enabled: bool,
    pub has_token: bool,
    pub last_started_at: Option<String>,
    pub last_finished_at: Option<String>,
    pub running: bool,
    pub last_run: Option<MultiplierMonitorRunSummary>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MultiplierMonitorRunSummary {
    pub run_id: String,
    pub started_at: String,
    pub finished_at: String,
    pub total: i64,
    pub updated: i64,
    pub unchanged: i64,
    pub issues: i64,
    pub failed: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MultiplierMonitorAccountOption {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub status: String,
    pub multiplier: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MultiplierMonitorCheckResult {
    pub run_id: String,
    pub total: i64,
    pub updated: i64,
    pub unchanged: i64,
    pub issues: i64,
    pub failed: i64,
}

#[derive(Debug, Deserialize, Default)]
pub struct MultiplierMonitorLogQuery {
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    pub result: Option<String>,
}
