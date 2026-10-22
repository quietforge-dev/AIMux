use crate::{
    config::Settings,
    error::AppError,
    model::account::Account,
    upstream::{proxy, timeout},
};
use reqwest::{Client, Response, Url};
use serde_json::Value;
use std::time::Duration;

pub fn client_with_timeout(
    settings: &Settings,
    request_timeout: Duration,
) -> Result<Client, AppError> {
    let mut builder = Client::builder().timeout(request_timeout);
    if let Some(p) = proxy::proxy(settings).map_err(|e| AppError::Internal(e.to_string()))? {
        builder = builder.proxy(p);
    }
    builder
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))
}
pub async fn post(
    account: &Account,
    endpoint: &str,
    body: &Value,
    settings: &Settings,
    headers: &[(String, String)],
) -> Result<Response, AppError> {
    post_with_timeout(
        account,
        endpoint,
        body,
        settings,
        headers,
        timeout::request_timeout(settings),
    )
    .await
}

pub async fn post_with_timeout(
    account: &Account,
    endpoint: &str,
    body: &Value,
    settings: &Settings,
    headers: &[(String, String)],
    request_timeout: Duration,
) -> Result<Response, AppError> {
    let url = upstream_url(&account.base_url, endpoint)?;
    tracing::info!(
        account_id = %account.id,
        account_name = %account.name,
        account_type = %account.r#type,
        endpoint,
        upstream_url = %url,
        "发送上游请求"
    );
    let client = client_with_timeout(settings, request_timeout)?;
    let mut req = client
        .post(url.clone())
        .bearer_auth(&account.api_key_encrypted)
        .json(body);
    for (key, value) in headers {
        req = req.header(key, value);
    }
    let response = req.send().await.map_err(|error| {
        tracing::error!(
            account_id = %account.id,
            account_name = %account.name,
            upstream_url = %url,
            %error,
            "上游请求连接失败"
        );
        if error.is_timeout() {
            AppError::UpstreamTimeout(error.to_string())
        } else {
            AppError::Upstream(error.to_string())
        }
    })?;
    tracing::info!(
        account_id = %account.id,
        account_name = %account.name,
        upstream_url = %url,
        status_code = response.status().as_u16(),
        "收到上游响应"
    );
    Ok(response)
}

pub fn upstream_url(base_url: &str, endpoint: &str) -> Result<Url, AppError> {
    let mut url = Url::parse(base_url.trim())
        .map_err(|error| AppError::BadRequest(format!("上游地址无效: {error}")))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::BadRequest(
            "上游地址必须使用 http 或 https 协议".into(),
        ));
    }

    let base_path = url.path().trim_end_matches('/');
    let endpoint_path = endpoint.trim().trim_start_matches('/');
    let endpoint_path = if base_path == "/v1" || base_path.ends_with("/v1") {
        if endpoint_path == "v1" {
            ""
        } else {
            endpoint_path.strip_prefix("v1/").unwrap_or(endpoint_path)
        }
    } else {
        endpoint_path
    };
    let path = match (base_path, endpoint_path) {
        ("" | "/", "") => "/".to_owned(),
        ("" | "/", endpoint) => format!("/{endpoint}"),
        (base, "") => base.to_owned(),
        (base, endpoint) => format!("{base}/{endpoint}"),
    };
    url.set_path(&path);
    Ok(url)
}
