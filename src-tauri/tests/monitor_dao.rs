use aimux_lib::{
    dao::{
        account_dao,
        monitor_dao::{create_and_refresh_account_average, list_grouped},
    },
    database::connect,
    model::monitor_record::MonitorRecord,
    schema::account_schema::AccountCreate,
};

fn record(account_id: &str, second: u8) -> MonitorRecord {
    MonitorRecord {
        id: uuid::Uuid::new_v4().to_string(),
        account_id: account_id.to_owned(),
        account_name: account_id.to_owned(),
        account_type: "openai".to_owned(),
        model: Some("gpt-5.6".to_owned()),
        checked_at: format!("2026-08-17T00:00:{second:02}Z"),
        duration_ms: Some(100),
        success: true,
        status_code: Some(200),
        error_code: None,
        error_message: None,
    }
}

async fn insert(pool: &sqlx::SqlitePool, r: &MonitorRecord) {
    sqlx::query("INSERT INTO monitor_records(id,account_id,account_name,account_type,model,checked_at,duration_ms,success,status_code,error_code,error_message) VALUES(?,?,?,?,?,?,?,?,?,?,?)")
        .bind(&r.id).bind(&r.account_id).bind(&r.account_name).bind(&r.account_type).bind(&r.model).bind(&r.checked_at).bind(r.duration_ms).bind(r.success).bind(r.status_code).bind(&r.error_code).bind(&r.error_message).execute(pool).await
        .expect("写入监控记录失败");
}

#[tokio::test]
async fn returns_the_latest_limit_for_each_account() {
    let path = std::env::temp_dir().join(format!(
        "aimux-monitor-records-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let pool = connect(&path).await.expect("创建数据库失败");
    for account_id in ["account-a", "account-b"] {
        for second in 1..=3 {
            insert(&pool, &record(account_id, second)).await;
        }
    }
    let ids = vec!["account-a".to_owned(), "account-b".to_owned()];
    let records = list_grouped(&pool, &ids, 2)
        .await
        .expect("读取监控记录失败");
    assert_eq!(records.len(), 4);
    for account_id in &ids {
        let account_records = records
            .iter()
            .filter(|record| record.account_id == *account_id)
            .collect::<Vec<_>>();
        assert_eq!(account_records.len(), 2);
        assert!(
            account_records
                .iter()
                .all(|record| record.checked_at.ends_with("02Z")
                    || record.checked_at.ends_with("03Z"))
        );
    }
    assert_eq!(list_grouped(&pool, &ids, 40).await.unwrap().len(), 6);
    assert!(list_grouped(&pool, &[], 30).await.unwrap().is_empty());
    pool.close().await;
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn stores_the_rolling_average_of_the_latest_thirty_records() {
    let path = std::env::temp_dir().join(format!(
        "aimux-monitor-average-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let pool = connect(&path).await.expect("创建数据库失败");
    let account = account_dao::create(
        &pool,
        AccountCreate {
            name: "monitor-average".into(),
            account_type: "openai".into(),
            base_url: "https://example.test".into(),
            api_key: "key".into(),
            status: "active".into(),
            priority: 5,
            multiplier: 0.10,
            test_default_model: None,
            model_mappings: None,
            supported_models: None,
            tags: None,
            notes: None,
        },
    )
    .await
    .expect("创建账号失败");
    for second in 1..=31 {
        let mut item = record(&account.id, second);
        item.duration_ms = Some(second as i64);
        create_and_refresh_account_average(&pool, &item)
            .await
            .expect("写入监控记录并刷新平均耗时失败");
    }
    let saved = account_dao::get(&pool, &account.id)
        .await
        .expect("读取账号失败")
        .expect("账号不存在");
    assert_eq!(saved.monitor_average_duration_ms, Some(17));
    let ids = vec![account.id.clone()];
    assert_eq!(list_grouped(&pool, &ids, 10).await.unwrap().len(), 10);
    assert_eq!(list_grouped(&pool, &ids, 40).await.unwrap().len(), 31);
    pool.close().await;
    let _ = std::fs::remove_file(path);
}
