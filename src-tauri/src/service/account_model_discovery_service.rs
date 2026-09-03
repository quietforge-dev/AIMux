use crate::{
    config::Settings,
    error::AppError,
    upstream::{client, error::response_body, timeout},
};
use reqwest::StatusCode;
use serde_json::Value;
use std::collections::BTreeSet;

pub async fn discover(
    account_type: &str,
    base_url: &str,
    api_key: &str,
    settings: &Settings,
) -> Result<Vec<String>, AppError> {
    let account_type = account_type.trim();
    if !matches!(account_type, "openai" | "anthropic") {
        return Err(AppError::BadRequest(
            "账号类型必须是 openai 或 anthropic".into(),
        ));
    }
    if base_url.trim().is_empty() {
        return Err(AppError::BadRequest("上游地址不能为空".into()));
    }
    if api_key.trim().is_empty() {
        return Err(AppError::BadRequest("API 密钥不能为空".into()));
    }

    let mut url = client::upstream_url(base_url, "v1/models")?;
    if account_type == "anthropic" {
        url.query_pairs_mut().append_pair("limit", "1000");
    }
    tracing::info!(account_type, upstream_url = %url, "查询上游模型列表");

    let http = client::client_with_timeout(settings, timeout::request_timeout(settings))?;
    let request = if account_type == "anthropic" {
        http.get(url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
    } else {
        http.get(url).bearer_auth(api_key)
    };
    let response = request.send().await.map_err(|error| {
        tracing::error!(account_type, %error, "查询上游模型列表失败");
        AppError::Upstream(error.to_string())
    })?;
    let status = response.status();
    let bytes = response.bytes().await.map_err(|error| {
        tracing::error!(account_type, %error, "读取上游模型列表响应失败");
        AppError::Upstream(error.to_string())
    })?;
    if !status.is_success() {
        return Err(upstream_status_error(status, &bytes));
    }
    let body: Value = serde_json::from_slice(&bytes)
        .map_err(|_| AppError::Upstream("上游返回的模型列表不是有效 JSON".into()))?;
    parse_models(&body)
}

fn upstream_status_error(status: StatusCode, bytes: &[u8]) -> AppError {
    let detail = response_body(bytes).trim().to_owned();
    let message = if detail.is_empty() {
        format!("查询模型列表失败，HTTP {}", status.as_u16())
    } else {
        format!("查询模型列表失败，HTTP {}：{detail}", status.as_u16())
    };
    AppError::Upstream(message)
}

fn parse_models(body: &Value) -> Result<Vec<String>, AppError> {
    let data = body
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Upstream("上游返回中未找到 data 模型列表".into()))?;
    let models = data
        .iter()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::parse_models;
    use serde_json::json;

    #[test]
    fn parses_unique_model_ids_in_order() {
        let models = parse_models(&json!({
            "data": [{"id": "model-b"}, {"id": " model-a "}, {"id": "model-b"}, {}]
        }))
        .unwrap();
        assert_eq!(models, ["model-a", "model-b"]);
    }

    #[test]
    fn rejects_unrecognised_response() {
        assert!(parse_models(&json!({"models": []})).is_err());
    }
}
