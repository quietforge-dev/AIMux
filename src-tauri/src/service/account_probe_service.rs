use std::time::Duration;

use reqwest::Response;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::{
    config::Settings, dao::model_dao, error::AppError, model::account::Account,
    service::account_service, upstream::client,
};

pub struct AccountProbe {
    pub model: String,
    pub endpoint: &'static str,
    pub body: Value,
}

pub async fn prepare(
    pool: &SqlitePool,
    account: &Account,
    requested_model: Option<&str>,
) -> Result<Option<AccountProbe>, AppError> {
    let model = match requested_model {
        Some(model) => Some(model.to_owned()),
        None => account
            .test_default_model
            .clone()
            .or(model_dao::default_name(pool, &account.r#type).await?),
    };
    Ok(model.map(|model| build_request(account, &model)))
}

pub fn build_request(account: &Account, model: &str) -> AccountProbe {
    build_request_with_stream(account, model, "ping", false)
}

pub fn build_stream_request(account: &Account, model: &str, prompt: &str) -> AccountProbe {
    build_request_with_stream(account, model, prompt, true)
}

fn build_request_with_stream(
    account: &Account,
    model: &str,
    prompt: &str,
    stream: bool,
) -> AccountProbe {
    let upstream_model =
        account_service::mapping(account, Some(model)).unwrap_or_else(|| model.into());
    let endpoint = if account.r#type == "anthropic" {
        "/v1/messages"
    } else {
        "/v1/chat/completions"
    };
    let mut body = json!({
        "model": upstream_model,
        "max_tokens": 1,
        "messages": [{"role": "user", "content": prompt}],
    });
    if stream {
        body["stream"] = serde_json::Value::Bool(true);
        // Leave room for reasoning before visible output; the probe stops at first text.
        body["max_tokens"] = serde_json::json!(256);
    }
    AccountProbe {
        model: model.to_owned(),
        endpoint,
        body,
    }
}

pub async fn send(
    account: &Account,
    probe: &AccountProbe,
    settings: &Settings,
    timeout: Option<Duration>,
) -> Result<Response, AppError> {
    match timeout {
        Some(timeout) => {
            client::post_with_timeout(account, probe.endpoint, &probe.body, settings, &[], timeout)
                .await
        }
        None => client::post(account, probe.endpoint, &probe.body, settings, &[]).await,
    }
}
