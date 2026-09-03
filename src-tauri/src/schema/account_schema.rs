use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct AccountCreate {
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default = "default_priority")]
    pub priority: i64,
    #[serde(default = "default_multiplier")]
    pub multiplier: f64,
    pub test_default_model: Option<String>,
    pub model_mappings: Option<serde_json::Value>,
    pub supported_models: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub notes: Option<String>,
}
fn default_status() -> String {
    "active".into()
}
fn default_priority() -> i64 {
    5
}
fn default_multiplier() -> f64 {
    0.10
}

#[derive(Debug, Deserialize, Default)]
pub struct AccountUpdate {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub account_type: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub status: Option<String>,
    pub priority: Option<i64>,
    pub multiplier: Option<f64>,
    pub test_default_model: Option<String>,
    pub model_mappings: Option<serde_json::Value>,
    pub supported_models: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AccountView {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub base_url: String,
    pub api_key: String,
    pub status: String,
    pub priority: i64,
    pub multiplier: f64,
    pub test_default_model: Option<String>,
    pub model_mappings: Option<serde_json::Value>,
    pub supported_models: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub notes: Option<String>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub last_successful_test_model: Option<String>,
    pub last_used_at: Option<String>,
    pub total_requests: i64,
    pub total_tokens: i64,
    pub monitor_average_duration_ms: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct TestRequest {
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DiscoverModelsRequest {
    #[serde(rename = "type")]
    pub account_type: String,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Serialize)]
pub struct DiscoverModelsResponse {
    pub models: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TestResult {
    pub account_id: String,
    pub success: bool,
    pub status_code: Option<u16>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub response_body: Option<String>,
    pub model: Option<String>,
}
