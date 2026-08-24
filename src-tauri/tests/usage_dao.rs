use aimux_lib::{dao::usage_dao, database::connect, model::usage_record::UsageRecord};

fn record(id: &str, account_id: &str, model: &str, success: bool, started_at: &str) -> UsageRecord {
    UsageRecord {
        id: id.into(),
        trace_id: format!("trace-{id}"),
        started_at: started_at.into(),
        ended_at: None,
        duration_ms: Some(100),
        first_token_ms: None,
        account_id: Some(account_id.into()),
        account_name: Some(account_id.into()),
        account_type: Some("openai".into()),
        model: Some(model.into()),
        reasoning_effort: None,
        endpoint: Some("/v1/chat/completions".into()),
        stream: false,
        success,
        status_code: Some(200),
        error_code: None,
        error_message: None,
        input_tokens: Some(10),
        output_tokens: Some(5),
        total_tokens: Some(15),
        cached_tokens: Some(3),
        client_ip: None,
        attempts: 1,
    }
}

#[tokio::test]
async fn applies_the_same_filters_to_usage_list_and_summary() {
    let path = std::env::temp_dir().join(format!("aimux-usage-{}.sqlite3", uuid::Uuid::new_v4()));
    let pool = connect(&path).await.expect("创建数据库失败");
    for usage in [
        record(
            "match",
            "account-a",
            "gpt-5.6",
            true,
            "2026-08-24T10:00:00Z",
        ),
        record(
            "wrong-status",
            "account-a",
            "gpt-5.6",
            false,
            "2026-08-24T10:00:01Z",
        ),
        record(
            "wrong-model",
            "account-a",
            "other",
            true,
            "2026-08-24T10:00:02Z",
        ),
        record(
            "wrong-account",
            "account-b",
            "gpt-5.6",
            true,
            "2026-08-24T10:00:03Z",
        ),
    ] {
        usage_dao::create(&pool, &usage)
            .await
            .expect("写入使用记录失败");
    }

    let filter = (
        Some("account-a"),
        Some("gpt-5.6"),
        Some("openai"),
        Some(true),
        Some("2026-08-24T00:00:00Z"),
        Some("2026-08-24T23:59:59Z"),
    );
    let (items, total) = usage_dao::list(
        &pool, 0, 20, filter.0, filter.1, filter.2, filter.3, filter.4, filter.5,
    )
    .await
    .expect("查询使用记录失败");
    let (count, successes, _, tokens) = usage_dao::summary(
        &pool, filter.0, filter.1, filter.2, filter.3, filter.4, filter.5,
    )
    .await
    .expect("汇总使用记录失败");

    assert_eq!(total, 1);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "match");
    assert_eq!(count, 1);
    assert_eq!(successes, 1);
    assert_eq!(tokens, 15);

    pool.close().await;
    let _ = std::fs::remove_file(path);
}
