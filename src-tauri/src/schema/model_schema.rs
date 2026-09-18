use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ModelCreate {
    pub name: String,
    #[serde(rename = "type")]
    pub model_type: String,
    pub provider: String,
}
#[derive(Debug, Deserialize, Default)]
pub struct ModelUpdate {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub model_type: Option<String>,
    pub provider: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ModelView {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub model_type: String,
    pub provider: String,
    pub is_default: i64,
    pub created_at: String,
    pub updated_at: String,
}
