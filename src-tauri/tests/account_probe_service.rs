use aimux_lib::{model::account::Account, service::account_probe_service::build_request};

fn account(account_type: &str, model_mappings: Option<&str>) -> Account {
    Account {
        id: "account-id".into(),
        name: "测试账号".into(),
        r#type: account_type.into(),
        base_url: "https://example.test".into(),
        api_key_encrypted: "key".into(),
        status: "active".into(),
        priority: 5,
        multiplier: 0.10,
        supported_models: None,
        tags: None,
        notes: None,
        last_error_code: None,
        last_error_message: None,
        last_successful_test_model: None,
        last_used_at: None,
        total_requests: 0,
        total_tokens: 0,
        monitor_average_duration_ms: None,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
        test_default_model: None,
        model_mappings: model_mappings.map(str::to_owned),
    }
}

#[test]
fn builds_openai_probe_with_mapped_model() {
    let probe = build_request(
        &account("openai", Some(r#"{"gpt-5.6":"upstream-model"}"#)),
        "gpt-5.6",
    );

    assert_eq!(probe.model, "gpt-5.6");
    assert_eq!(probe.endpoint, "/v1/chat/completions");
    assert_eq!(probe.body["model"], "upstream-model");
    assert_eq!(probe.body["messages"][0]["content"], "ping");
    assert!(probe.body.get("reasoning_effort").is_none());
}

#[test]
fn builds_anthropic_probe_with_requested_model() {
    let probe = build_request(&account("anthropic", None), "claude-test");

    assert_eq!(probe.endpoint, "/v1/messages");
    assert_eq!(probe.body["model"], "claude-test");
}
