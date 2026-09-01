use aimux_lib::{
    dao::account_dao::{adjust, create, get, list, pick_one},
    database::connect,
    schema::account_schema::AccountCreate,
};

fn input(name: &str, base_url: String) -> AccountCreate {
    AccountCreate {
        name: name.into(),
        account_type: "openai".into(),
        base_url,
        api_key: "key".into(),
        status: "active".into(),
        priority: 5,
        multiplier: 0.10,
        test_default_model: None,
        model_mappings: None,
        supported_models: None,
        tags: None,
        notes: None,
    }
}

#[tokio::test]
async fn creates_account_with_current_columns() {
    let path = std::env::temp_dir().join(format!("aimux-account-{}.sqlite3", uuid::Uuid::new_v4()));
    let pool = connect(&path).await.expect("创建数据库失败");
    let account = create(&pool, input("test", "https://example.test".into()))
        .await
        .expect("创建账号失败");
    assert_eq!(
        get(&pool, &account.id)
            .await
            .expect("读取账号失败")
            .expect("账号不存在")
            .name,
        "test"
    );
    pool.close().await;
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn sorts_same_multiplier_by_monitor_average_duration_with_unknown_last() {
    let path = std::env::temp_dir().join(format!("aimux-account-{}.sqlite3", uuid::Uuid::new_v4()));
    let pool = connect(&path).await.expect("创建数据库失败");
    let mut accounts = Vec::new();
    for name in ["unknown", "slower", "faster"] {
        accounts.push(
            create(&pool, input(name, format!("https://{name}.example.test")))
                .await
                .expect("创建账号失败"),
        );
    }
    for (account, duration_ms) in [(&accounts[1], 2_000_i64), (&accounts[2], 1_000_i64)] {
        sqlx::query("UPDATE accounts SET monitor_average_duration_ms=? WHERE id=?")
            .bind(duration_ms)
            .bind(&account.id)
            .execute(&pool)
            .await
            .expect("写入平均耗时失败");
    }
    let (listed, _) = list(&pool, 0, 20, None, None, None)
        .await
        .expect("查询账号列表失败");
    assert_eq!(
        listed
            .iter()
            .map(|account| account.name.as_str())
            .collect::<Vec<_>>(),
        ["faster", "slower", "unknown"]
    );
    let (filtered, total) = list(&pool, 0, 20, None, None, Some("FAST"))
        .await
        .expect("按名称筛选账号失败");
    assert_eq!(total, 1);
    assert_eq!(filtered[0].name, "faster");
    assert_eq!(
        pick_one(&pool, None, "openai")
            .await
            .expect("选择账号失败")
            .expect("未选择到账号")
            .name,
        "faster"
    );
    pool.close().await;
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn gateway_failures_reduce_priority_by_two_while_other_failures_reduce_by_one() {
    let path = std::env::temp_dir().join(format!("aimux-account-{}.sqlite3", uuid::Uuid::new_v4()));
    let pool = connect(&path).await.expect("创建数据库失败");
    let account = create(&pool, input("priority", "https://example.test".into()))
        .await
        .expect("创建账号失败");

    adjust(
        &pool,
        &account.id,
        false,
        "gateway",
        Some("upstream_error"),
        None,
    )
    .await
    .expect("记录网关失败失败");
    assert_eq!(
        get(&pool, &account.id)
            .await
            .expect("读取账号失败")
            .expect("账号不存在")
            .priority,
        3
    );

    adjust(
        &pool,
        &account.id,
        false,
        "request",
        Some("test_failed"),
        None,
    )
    .await
    .expect("记录测试失败失败");
    assert_eq!(
        get(&pool, &account.id)
            .await
            .expect("读取账号失败")
            .expect("账号不存在")
            .priority,
        2
    );

    adjust(
        &pool,
        &account.id,
        false,
        "monitor",
        Some("monitor_failed"),
        None,
    )
    .await
    .expect("记录监控失败失败");
    assert_eq!(
        get(&pool, &account.id)
            .await
            .expect("读取账号失败")
            .expect("账号不存在")
            .priority,
        1
    );

    pool.close().await;
    let _ = std::fs::remove_file(path);
}
