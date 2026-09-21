use std::collections::HashMap;

use aimux_lib::{
    config::Settings,
    dao::{account_dao, multiplier_monitor_dao},
    database,
    schema::{account_schema::AccountCreate, multiplier_monitor_schema::MultiplierMonitorCreate},
    service::multiplier_monitor_service,
};
use axum::{extract::Query, routing::get, Json, Router};
use serde_json::json;
use tokio::net::TcpListener;

async fn multiplier_response(
    Query(query): Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let page = query
        .get("page")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_default();
    Json(json!({
        "code": 0,
        "message": "ok",
        "data": {
            "items": if page == 1 {
                vec![json!({"key":"unrelated-key","group":{"rate_multiplier":0.15}})]
            } else if page == 2 {
                vec![json!({"key":"key-matched","group":{"rate_multiplier":0.25}})]
            } else {
                Vec::new()
            },
            "total": 2,
            "page": page,
            "page_size": 100,
            "pages": 2
        }
    }))
}

fn account(name: &str, api_key: &str) -> AccountCreate {
    AccountCreate {
        name: name.into(),
        account_type: "openai".into(),
        base_url: "https://example.com/v1".into(),
        api_key: api_key.into(),
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
async fn checks_remote_rates_and_only_updates_the_matching_account() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route("/keys", get(multiplier_response)),
        )
        .await
        .unwrap();
    });

    let path = std::env::temp_dir().join(format!(
        "aimux-multiplier-monitor-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let pool = database::connect(&path).await.expect("创建测试数据库");
    let matched = account_dao::create(&pool, account("匹配账号", "key-matched"))
        .await
        .expect("创建匹配账号");
    let missing = account_dao::create(&pool, account("未匹配账号", "key-missing"))
        .await
        .expect("创建未匹配账号");
    let config = multiplier_monitor_service::create(
        &pool,
        MultiplierMonitorCreate {
            name: "测试倍率接口".into(),
            url: format!("http://{address}/keys"),
            token: "test-token".into(),
            account_ids: vec![matched.id.clone(), missing.id.clone()],
            enabled: true,
        },
    )
    .await
    .expect("创建倍率监控配置");

    let result = multiplier_monitor_service::check_now(&pool, &Settings::default(), &config.id)
        .await
        .expect("执行倍率检查");
    assert_eq!(result.updated, 1);
    assert_eq!(result.issues, 1);
    assert_eq!(
        account_dao::get(&pool, &matched.id)
            .await
            .unwrap()
            .unwrap()
            .multiplier,
        0.25
    );
    assert_eq!(
        account_dao::get(&pool, &missing.id)
            .await
            .unwrap()
            .unwrap()
            .multiplier,
        0.10
    );
    let (logs, total) = multiplier_monitor_dao::list_logs(&pool, &config.id, 0, 20, None)
        .await
        .expect("读取倍率日志");
    assert_eq!(total, 2);
    assert!(logs.iter().any(|log| log.result == "updated"));
    assert!(logs.iter().any(|log| log.result == "missing_key"));

    pool.close().await;
    server.abort();
    let _ = std::fs::remove_file(path);
}
