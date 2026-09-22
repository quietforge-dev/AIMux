use std::{fmt, time::Duration};

use futures_util::StreamExt;
use reqwest::{
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    redirect::Policy,
    Client, Response, Url,
};
use serde::Deserialize;
use serde_json::Value;

use crate::{config::Settings, upstream::proxy};

const PAGE_SIZE: usize = 100;
const QUERY_TIMEOUT: Duration = Duration::from_secs(40);
const MAX_PAGE_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_PAGES: usize = 1_000;
const MAX_ITEMS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteMultiplier {
    pub key: String,
    pub rate_multiplier: Option<String>,
    pub rate_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MultiplierQueryError {
    pub code: String,
    pub message: String,
    pub http_status: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshedTokens {
    pub access_token: String,
    pub refresh_token: String,
}

impl MultiplierQueryError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            http_status: None,
        }
    }

    fn with_status(code: &str, message: impl Into<String>, status: u16) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            http_status: Some(status as i64),
        }
    }
}

impl fmt::Display for MultiplierQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for MultiplierQueryError {}

#[derive(Debug, Deserialize)]
struct RootResponse {
    code: Option<i64>,
    data: Option<PageData>,
}

#[derive(Debug, Deserialize)]
struct PageData {
    items: Vec<RemoteItem>,
    total: usize,
    page: usize,
    page_size: usize,
    pages: usize,
}

#[derive(Debug, Deserialize)]
struct RemoteItem {
    key: Option<String>,
    group: Option<RemoteGroup>,
}

#[derive(Debug, Deserialize)]
struct RemoteGroup {
    rate_multiplier: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct RefreshResponse {
    code: Option<i64>,
    data: Option<RefreshData>,
}

#[derive(Debug, Deserialize)]
struct RefreshData {
    access_token: Option<String>,
    refresh_token: Option<String>,
}

struct ParsedPage {
    items: Vec<RemoteMultiplier>,
    raw_item_count: usize,
    total: usize,
    pages: usize,
}

pub async fn fetch_all(
    raw_url: &str,
    raw_token: &str,
    settings: &Settings,
) -> Result<Vec<RemoteMultiplier>, MultiplierQueryError> {
    let base_url = validated_url(raw_url)?;
    let authorization = authorization_value(raw_token)?;
    let http = build_client(settings)?;
    let mut result = Vec::new();
    let mut received_items = 0usize;
    let mut page = 1usize;
    let mut expected_total = None;
    let mut expected_pages = None;

    loop {
        if page > MAX_PAGES {
            return Err(MultiplierQueryError::new(
                "too_many_pages",
                "倍率查询分页超过安全上限",
            ));
        }
        let response = http
            .get(with_page_query(
                &endpoint_url(&base_url, "/api/v1/keys", false),
                page,
            ))
            .header(AUTHORIZATION, &authorization)
            .header(ACCEPT, "application/json")
            .send()
            .await
            .map_err(|_| {
                MultiplierQueryError::new(
                    "request_failed",
                    "倍率查询请求失败，请检查地址、网络、代理或 TLS 配置",
                )
            })?;
        let status = response.status();
        if !status.is_success() {
            let status_code = status.as_u16();
            return Err(MultiplierQueryError::with_status(
                if status_code == 401 {
                    "unauthorized"
                } else {
                    "http_error"
                },
                format!("倍率查询接口返回 HTTP {status_code}"),
                status_code,
            ));
        }
        let bytes = read_limited(response, MAX_PAGE_RESPONSE_BYTES).await?;
        let parsed = parse_page(&bytes, page)?;
        let effective_pages = parsed.pages.max(1);
        match (expected_total, expected_pages) {
            (None, None) => {
                if parsed.total > MAX_ITEMS || effective_pages > MAX_PAGES {
                    return Err(MultiplierQueryError::new(
                        "response_too_large",
                        "倍率查询数据量超过安全上限",
                    ));
                }
                expected_total = Some(parsed.total);
                expected_pages = Some(effective_pages);
            }
            (Some(total), Some(pages)) if total != parsed.total || pages != effective_pages => {
                return Err(MultiplierQueryError::new(
                    "pagination_changed",
                    "倍率查询期间 total/pages 发生变化",
                ));
            }
            _ => {}
        }
        received_items = received_items
            .checked_add(parsed.raw_item_count)
            .ok_or_else(|| MultiplierQueryError::new("item_count_overflow", "倍率查询条数溢出"))?;
        if received_items > MAX_ITEMS {
            return Err(MultiplierQueryError::new(
                "response_too_large",
                "倍率查询条数超过安全上限",
            ));
        }
        result.extend(parsed.items);
        if page >= expected_pages.unwrap_or(1) {
            if Some(received_items) != expected_total {
                return Err(MultiplierQueryError::new(
                    "total_mismatch",
                    "倍率查询实际条数与 total 不一致",
                ));
            }
            break;
        }
        page += 1;
    }
    Ok(result)
}

pub async fn refresh_tokens(
    raw_url: &str,
    raw_refresh_token: &str,
    settings: &Settings,
) -> Result<RefreshedTokens, MultiplierQueryError> {
    let base_url = validated_url(raw_url)?;
    let refresh_token = raw_refresh_token.trim();
    if refresh_token.is_empty() {
        return Err(MultiplierQueryError::new(
            "empty_refresh_token",
            "refresh token 不能为空",
        ));
    }
    let http = build_client(settings)?;
    let response = http
        .post(endpoint_url(&base_url, "/api/v1/auth/refresh", true))
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .json(&serde_json::json!({ "refresh_token": refresh_token }))
        .send()
        .await
        .map_err(|_| {
            MultiplierQueryError::new(
                "refresh_request_failed",
                "刷新倍率查询 token 请求失败，请检查地址、网络、代理或 TLS 配置",
            )
        })?;
    let status = response.status();
    if !status.is_success() {
        let status_code = status.as_u16();
        return Err(MultiplierQueryError::with_status(
            if status_code == 401 {
                "refresh_unauthorized"
            } else {
                "refresh_http_error"
            },
            format!("刷新 token 接口返回 HTTP {status_code}"),
            status_code,
        ));
    }
    let bytes = read_limited(response, MAX_PAGE_RESPONSE_BYTES).await?;
    let root: RefreshResponse = serde_json::from_slice(&bytes).map_err(|_| {
        MultiplierQueryError::new("refresh_invalid_json", "刷新 token 接口返回的不是有效 JSON")
    })?;
    if root.code.is_some_and(|code| !matches!(code, 0 | 200)) {
        return Err(MultiplierQueryError::new(
            "refresh_business_error",
            format!(
                "刷新 token 业务失败，code={}",
                root.code.unwrap_or_default()
            ),
        ));
    }
    let Some(data) = root.data else {
        return Err(MultiplierQueryError::new(
            "refresh_invalid_response",
            "刷新 token 响应缺少 data",
        ));
    };
    let access_token = normalize_access_token(data.access_token.as_deref()).ok_or_else(|| {
        MultiplierQueryError::new(
            "refresh_invalid_response",
            "刷新 token 响应缺少 access_token",
        )
    })?;
    let refresh_token = data
        .refresh_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            MultiplierQueryError::new(
                "refresh_invalid_response",
                "刷新 token 响应缺少 refresh_token",
            )
        })?;
    Ok(RefreshedTokens {
        access_token,
        refresh_token,
    })
}

pub fn validate_url(raw_url: &str) -> Result<(), MultiplierQueryError> {
    validated_url(raw_url).map(|_| ())
}

pub fn normalize_base_url(raw_url: &str) -> Result<String, MultiplierQueryError> {
    let mut url = validated_url(raw_url)?;
    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string().trim_end_matches('/').to_owned())
}

fn validated_url(raw_url: &str) -> Result<Url, MultiplierQueryError> {
    let url = Url::parse(raw_url.trim()).map_err(|error| {
        MultiplierQueryError::new("invalid_url", format!("倍率查询 URL 无效: {error}"))
    })?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(MultiplierQueryError::new(
            "invalid_url",
            "倍率查询 URL 必须使用 http 或 https 且包含主机",
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(MultiplierQueryError::new(
            "url_contains_credentials",
            "倍率查询 URL 不能包含用户名或密码",
        ));
    }
    if url.fragment().is_some() {
        return Err(MultiplierQueryError::new(
            "url_contains_fragment",
            "倍率查询 URL 不能包含 fragment",
        ));
    }
    let sensitive_query_names = ["token", "access_token", "api_key", "authorization"];
    if url.query_pairs().any(|(name, _)| {
        sensitive_query_names
            .iter()
            .any(|sensitive| name.eq_ignore_ascii_case(sensitive))
    }) {
        return Err(MultiplierQueryError::new(
            "url_contains_token",
            "查询 Token 不能放在 URL 参数中，请填写到 Token 字段",
        ));
    }
    Ok(url)
}

fn authorization_value(raw_token: &str) -> Result<String, MultiplierQueryError> {
    let token = raw_token.trim();
    if token.is_empty() {
        return Err(MultiplierQueryError::new(
            "empty_token",
            "倍率查询 token 不能为空",
        ));
    }
    let has_bearer = token
        .get(..7)
        .map(|prefix| prefix.eq_ignore_ascii_case("Bearer "))
        .unwrap_or(false);
    Ok(if has_bearer {
        token.to_owned()
    } else {
        format!("Bearer {token}")
    })
}

fn normalize_access_token(raw_token: Option<&str>) -> Option<String> {
    let token = raw_token?.trim();
    if token.is_empty() {
        return None;
    }
    Some(
        token
            .get(7..)
            .filter(|_| token[..7].eq_ignore_ascii_case("Bearer "))
            .unwrap_or(token)
            .trim()
            .to_owned(),
    )
}

fn build_client(settings: &Settings) -> Result<Client, MultiplierQueryError> {
    let mut builder = Client::builder()
        .timeout(QUERY_TIMEOUT)
        .redirect(Policy::none());
    if let Some(configured_proxy) = proxy::proxy(settings)
        .map_err(|_| MultiplierQueryError::new("invalid_proxy", "代理配置无效"))?
    {
        builder = builder.proxy(configured_proxy);
    }
    builder.build().map_err(|error| {
        MultiplierQueryError::new(
            "client_build_failed",
            format!("创建查询客户端失败: {error}"),
        )
    })
}

fn with_page_query(base_url: &Url, page: usize) -> Url {
    let fixed = ["page", "page_size", "status", "timezone"];
    let original = base_url
        .query_pairs()
        .filter(|(name, _)| !fixed.iter().any(|fixed_name| name == *fixed_name))
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    let mut url = base_url.clone();
    url.set_query(None);
    {
        let mut query = url.query_pairs_mut();
        for (name, value) in original {
            query.append_pair(&name, &value);
        }
        query
            .append_pair("page", &page.to_string())
            .append_pair("page_size", &PAGE_SIZE.to_string())
            .append_pair("status", "active")
            .append_pair("timezone", "Asia/Shanghai");
    }
    url
}

fn endpoint_url(base_url: &Url, path: &str, clear_query: bool) -> Url {
    let mut url = base_url.clone();
    url.set_path(path);
    url.set_fragment(None);
    if clear_query {
        url.set_query(None);
    }
    url
}

async fn read_limited(response: Response, limit: usize) -> Result<Vec<u8>, MultiplierQueryError> {
    if response
        .content_length()
        .map(|length| length > limit as u64)
        .unwrap_or(false)
    {
        return Err(MultiplierQueryError::new(
            "response_too_large",
            "倍率查询响应超过大小限制",
        ));
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| {
            MultiplierQueryError::new(
                "response_read_failed",
                format!("读取倍率查询响应失败: {error}"),
            )
        })?;
        if bytes.len() + chunk.len() > limit {
            return Err(MultiplierQueryError::new(
                "response_too_large",
                "倍率查询响应超过大小限制",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn parse_page(bytes: &[u8], requested_page: usize) -> Result<ParsedPage, MultiplierQueryError> {
    let root: RootResponse = serde_json::from_slice(bytes).map_err(|_| {
        MultiplierQueryError::new("invalid_json", "倍率查询接口返回的不是有效 JSON")
    })?;
    if root.code.is_some_and(|code| !matches!(code, 0 | 200)) {
        let code = root.code.unwrap_or_default();
        return Err(if code == 401 {
            MultiplierQueryError::with_status("unauthorized", "倍率查询业务返回 401", 401)
        } else {
            MultiplierQueryError::new("business_error", format!("倍率查询业务失败，code={code}"))
        });
    }
    let Some(data) = root.data else {
        return Err(MultiplierQueryError::new(
            "invalid_response",
            "倍率查询接口返回缺少 data",
        ));
    };
    if data.page != requested_page || data.page_size != PAGE_SIZE {
        return Err(MultiplierQueryError::new(
            "pagination_mismatch",
            "倍率查询分页信息不一致",
        ));
    }
    let raw_item_count = data.items.len();
    let items = data
        .items
        .into_iter()
        .filter_map(|item| {
            let key = item.key?.trim().to_owned();
            if key.is_empty() {
                return None;
            }
            let (rate_multiplier, rate_error) =
                extract_rate(item.group.and_then(|group| group.rate_multiplier));
            Some(RemoteMultiplier {
                key,
                rate_multiplier,
                rate_error,
            })
        })
        .collect();
    Ok(ParsedPage {
        items,
        raw_item_count,
        total: data.total,
        pages: data.pages,
    })
}

fn extract_rate(value: Option<Value>) -> (Option<String>, Option<String>) {
    let Some(value) = value else {
        return (None, None);
    };
    let text = match value {
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.trim().to_owned(),
        _ => return (None, Some("rate_multiplier 不是数字".into())),
    };
    if text.is_empty() {
        (None, Some("rate_multiplier 为空".into()))
    } else {
        (Some(text), None)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        authorization_value, endpoint_url, normalize_access_token, normalize_base_url, parse_page,
        with_page_query, PAGE_SIZE,
    };
    use reqwest::Url;

    #[test]
    fn adds_bearer_prefix_once() {
        assert_eq!(authorization_value("abc").unwrap(), "Bearer abc");
        assert_eq!(authorization_value("bearer abc").unwrap(), "bearer abc");
    }

    #[test]
    fn replaces_fixed_query_parameters_and_keeps_others() {
        let url = Url::parse("https://example.com/keys?tenant=a&page=8").unwrap();
        let value = with_page_query(&url, 2);
        assert!(value.as_str().contains("tenant=a"));
        assert!(value.as_str().contains("page=2"));
        assert!(value.as_str().contains(&format!("page_size={PAGE_SIZE}")));
        assert!(!value.as_str().contains("page=8"));
    }

    #[test]
    fn parses_key_and_rate_without_float_conversion() {
        let body = br#"{
            "code": 0,
            "data": {
                "items": [{"key":" sk-test ","group":{"rate_multiplier":0.30}}],
                "total":1,"page":1,"page_size":100,"pages":1
            }
        }"#;
        let page = parse_page(body, 1).unwrap();
        assert_eq!(page.items[0].key, "sk-test");
        assert_eq!(page.items[0].rate_multiplier.as_deref(), Some("0.3"));
    }

    #[test]
    fn normalizes_domain_and_discards_legacy_path() {
        assert_eq!(
            normalize_base_url("https://example.com/api/v1/keys/").unwrap(),
            "https://example.com"
        );
    }

    #[test]
    fn builds_fixed_api_endpoints_from_legacy_or_domain_url() {
        let base = reqwest::Url::parse("https://example.com/old/path?tenant=a").unwrap();
        assert_eq!(
            endpoint_url(&base, "/api/v1/keys", false).as_str(),
            "https://example.com/api/v1/keys?tenant=a"
        );
        assert_eq!(
            endpoint_url(&base, "/api/v1/auth/refresh", true).as_str(),
            "https://example.com/api/v1/auth/refresh"
        );
    }

    #[test]
    fn recognizes_business_unauthorized_response_without_data() {
        let error = match parse_page(br#"{"code":401,"message":"unauthorized"}"#, 1) {
            Ok(_) => panic!("expected unauthorized error"),
            Err(error) => error,
        };
        assert_eq!(error.code, "unauthorized");
        assert_eq!(error.http_status, Some(401));
    }

    #[test]
    fn strips_bearer_prefix_from_refreshed_access_token() {
        assert_eq!(
            normalize_access_token(Some("Bearer access-token")).as_deref(),
            Some("access-token")
        );
        assert_eq!(normalize_access_token(Some("  ")), None);
    }
}
