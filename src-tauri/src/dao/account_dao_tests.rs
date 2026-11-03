use super::*;
use crate::service::account_service;
use serde_json::json;
use sqlx::sqlite::SqlitePoolOptions;

async fn pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    seed_models(&pool).await;
    pool
}

async fn seed_models(pool: &SqlitePool) {
    for (kind, provider, model) in [
        ("openai", "openai", "gpt-test"),
        ("openai", "deepseek", "deepseek-test"),
        ("anthropic", "anthropic", "claude-test"),
        ("openai", "anthropic", "claude-compatible"),
    ] {
        sqlx::query("INSERT INTO models(id,name,type,provider,is_default,created_at,updated_at) VALUES(?,?,?,?,0,'test','test')")
            .bind(model).bind(model).bind(kind).bind(provider).execute(pool).await.unwrap();
    }
}

async fn account(
    pool: &SqlitePool,
    name: &str,
    kind: &str,
    provider: Option<&str>,
    model: &str,
    priority: i64,
    multiplier: f64,
    super_priority: bool,
) -> Account {
    account_service::create(
        pool,
        serde_json::from_value(json!({
            "name": name, "type": kind, "provider": provider, "super_priority": super_priority,
            "base_url": "https://example.invalid/v1", "api_key": "fake-test-key",
            "priority": priority, "multiplier": multiplier, "supported_models": [model],
            "test_default_model": model
        }))
        .unwrap(),
    )
    .await
    .unwrap()
}

async fn selected(pool: &SqlitePool, model: Option<&str>, kind: &str) -> String {
    pick_one(pool, model, kind).await.unwrap().unwrap().id
}

#[tokio::test]
async fn super_priority_overrides_cost_and_score_but_preserves_normal_monitor_comparison() {
    let pool = pool().await;
    let ordinary = account(
        &pool,
        "cheap",
        "openai",
        Some("openai"),
        "gpt-test",
        9,
        0.1,
        false,
    )
    .await;
    let preferred = account(
        &pool,
        "preferred",
        "openai",
        Some("openai"),
        "gpt-test",
        5,
        0.8,
        true,
    )
    .await;
    assert_eq!(
        selected(&pool, Some("gpt-test"), "openai").await,
        preferred.id
    );
    assert_eq!(
        pick_one_normal(&pool, Some("gpt-test"), "openai")
            .await
            .unwrap()
            .unwrap()
            .id,
        ordinary.id
    );

    // 失败扣分仍走原逻辑，4 分回退；成功恢复到 5 后自动重新生效。
    account_service::record_failure(
        &pool,
        &preferred.id,
        &crate::config::Settings::default(),
        Some("first_token_timeout"),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        selected(&pool, Some("gpt-test"), "openai").await,
        ordinary.id
    );
    assert!(
        get(&pool, &preferred.id)
            .await
            .unwrap()
            .unwrap()
            .super_priority
    );
    account_service::record_monitor(&pool, &preferred.id, true, None, None)
        .await
        .unwrap();
    assert_eq!(
        selected(&pool, Some("gpt-test"), "openai").await,
        preferred.id
    );
    set_status(&pool, &preferred.id, "disabled", None)
        .await
        .unwrap();
    assert_eq!(
        selected(&pool, Some("gpt-test"), "openai").await,
        ordinary.id
    );
    set_status(&pool, &preferred.id, "active", None)
        .await
        .unwrap();
    assert_eq!(
        selected(&pool, Some("gpt-test"), "openai").await,
        preferred.id
    );
}

#[tokio::test]
async fn respects_provider_protocol_model_and_unknown_model_fallback() {
    let pool = pool().await;
    let ordinary = account(&pool, "ordinary", "openai", None, "gpt-test", 9, 0.1, false).await;
    let preferred = account(
        &pool,
        "preferred",
        "openai",
        Some("deepseek"),
        "gpt-test",
        5,
        0.8,
        true,
    )
    .await;
    assert_eq!(
        selected(&pool, Some("gpt-test"), "openai").await,
        ordinary.id
    );
    // 修改供应商后，按客户端模型归属而不是上游模型映射识别。
    let payload = serde_json::from_value(
        json!({"provider": "openai", "model_mappings": {"gpt-test": "deepseek-test"}}),
    )
    .unwrap();
    let preferred = account_service::update(&pool, preferred, payload)
        .await
        .unwrap();
    assert_eq!(
        selected(&pool, Some("gpt-test"), "openai").await,
        preferred.id
    );
    account(
        &pool,
        "claude",
        "anthropic",
        Some("anthropic"),
        "claude-test",
        5,
        0.8,
        true,
    )
    .await;
    assert_eq!(
        selected(&pool, Some("gpt-test"), "openai").await,
        preferred.id
    );
    assert!(pick_one(&pool, Some("deepseek-test"), "openai")
        .await
        .unwrap()
        .is_none());
    assert_eq!(selected(&pool, None, "openai").await, ordinary.id);
    let custom = account(
        &pool,
        "custom",
        "openai",
        Some("openai"),
        "custom-model",
        9,
        0.1,
        false,
    )
    .await;
    account_service::update(
        &pool,
        preferred,
        serde_json::from_value(json!({"supported_models": ["gpt-test", "custom-model"]})).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        selected(&pool, Some("custom-model"), "openai").await,
        custom.id
    );
}

#[tokio::test]
async fn create_update_and_toggle_enforce_scope_and_preserve_metadata() {
    let pool = pool().await;
    let a = account(
        &pool,
        "a",
        "openai",
        Some("openai"),
        "gpt-test",
        6,
        0.2,
        true,
    )
    .await;
    let b = account(
        &pool,
        "b",
        "openai",
        Some("openai"),
        "gpt-test",
        6,
        0.2,
        true,
    )
    .await;
    assert!(!get(&pool, &a.id).await.unwrap().unwrap().super_priority);
    let deepseek = account(
        &pool,
        "ds",
        "openai",
        Some("deepseek"),
        "deepseek-test",
        6,
        0.2,
        true,
    )
    .await;
    let anthropic = account(
        &pool,
        "anthropic",
        "anthropic",
        Some("anthropic"),
        "claude-test",
        6,
        0.2,
        true,
    )
    .await;
    let compatible = account(
        &pool,
        "compatible",
        "openai",
        Some("anthropic"),
        "claude-compatible",
        6,
        0.2,
        true,
    )
    .await;
    let b = account_service::update(
        &pool,
        b,
        serde_json::from_value(json!({"provider":"deepseek"})).unwrap(),
    )
    .await
    .unwrap();
    assert!(
        !get(&pool, &deepseek.id)
            .await
            .unwrap()
            .unwrap()
            .super_priority
    );
    assert!(
        get(&pool, &anthropic.id)
            .await
            .unwrap()
            .unwrap()
            .super_priority
    );
    assert!(
        get(&pool, &compatible.id)
            .await
            .unwrap()
            .unwrap()
            .super_priority
    );
    let b = account_service::update(
        &pool,
        b,
        serde_json::from_value(
            json!({"provider":"anthropic", "type":"anthropic", "supported_models":["claude-test"]}),
        )
        .unwrap(),
    )
    .await
    .unwrap();
    assert!(
        !get(&pool, &anthropic.id)
            .await
            .unwrap()
            .unwrap()
            .super_priority
    );
    assert!(
        get(&pool, &compatible.id)
            .await
            .unwrap()
            .unwrap()
            .super_priority
    );
    // 不包含新字段的旧 API 调用不能擦除标记，倍率/扣分写入也不会覆盖。
    let b = account_service::update(
        &pool,
        b,
        serde_json::from_value(json!({"notes":"legacy client", "multiplier":0.3})).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(b.provider.as_deref(), Some("anthropic"));
    assert!(b.super_priority);
    adjust_by(&pool, &b.id, -2, None, None).await.unwrap();
    assert!(get(&pool, &b.id).await.unwrap().unwrap().super_priority);
    let b = account_service::update(
        &pool,
        b,
        serde_json::from_value(json!({"provider":"", "super_priority":false})).unwrap(),
    )
    .await
    .unwrap();
    assert!(b.provider.is_none());
    assert!(!b.super_priority);
    assert!(set_super_priority(&pool, &b.id, true).await.is_err());
    assert!(set_super_priority(&pool, "missing", true).await.is_err());
    set_super_priority(&pool, &a.id, true).await.unwrap();
    assert!(get(&pool, &a.id).await.unwrap().unwrap().super_priority);
    set_super_priority(&pool, &a.id, false).await.unwrap();
    assert!(!get(&pool, &a.id).await.unwrap().unwrap().super_priority);
    let invalid = serde_json::from_value(json!({"provider":"", "super_priority":true})).unwrap();
    assert!(account_service::update(&pool, a.clone(), invalid)
        .await
        .is_err());
    assert_eq!(
        get(&pool, &a.id).await.unwrap().unwrap().provider,
        a.provider
    );
}

#[tokio::test]
async fn migration_backfills_only_unambiguous_complete_model_lists() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let migrator = sqlx::migrate!("./migrations");
    for migration in migrator.iter().filter(|migration| migration.version < 13) {
        sqlx::raw_sql(&migration.sql).execute(&pool).await.unwrap();
    }
    seed_models(&pool).await;
    for (id, models) in [
        ("known", Some(r#"["gpt-test"]"#)),
        ("mixed", Some(r#"["gpt-test","deepseek-test"]"#)),
        ("partial", Some(r#"["gpt-test","custom"]"#)),
        ("custom", Some(r#"["custom"]"#)),
        ("empty", Some("[]")),
        ("invalid", Some("bad json")),
        ("object", Some(r#"{"model":"gpt-test"}"#)),
        ("null", None),
    ] {
        sqlx::query("INSERT INTO accounts(id,name,type,base_url,api_key_encrypted,status,priority,multiplier,total_requests,total_tokens,created_at,updated_at,supported_models) VALUES(?,?,'openai','https://example.invalid','fake','active',8,0.8,10,500,'old','old',?)")
            .bind(id).bind(id).bind(models).execute(&pool).await.unwrap();
    }
    sqlx::raw_sql(include_str!(
        "../../migrations/0013_accounts_provider_super_priority.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let known = get(&pool, "known").await.unwrap().unwrap();
    assert_eq!(known.provider.as_deref(), Some("openai"));
    assert!(!known.super_priority);
    assert_eq!(
        (known.priority, known.multiplier, known.total_tokens),
        (8, 0.8, 500)
    );
    for id in [
        "mixed", "partial", "custom", "empty", "invalid", "object", "null",
    ] {
        let row = get(&pool, id).await.unwrap().unwrap();
        assert!(row.provider.is_none(), "{id}");
        assert!(!row.super_priority);
    }
}

#[tokio::test]
async fn concurrent_enables_are_serialized_without_unique_constraint() {
    let folder = std::env::temp_dir().join(format!("aimux-super-test-{}", Uuid::new_v4()));
    tokio::fs::create_dir(&folder).await.unwrap();
    let path = folder.join("test.db");
    let pool = crate::database::connect(&path).await.unwrap();
    seed_models(&pool).await;
    let a = account(
        &pool,
        "a",
        "openai",
        Some("openai"),
        "gpt-test",
        5,
        0.8,
        false,
    )
    .await;
    let b = account(
        &pool,
        "b",
        "openai",
        Some("openai"),
        "gpt-test",
        5,
        0.8,
        false,
    )
    .await;
    let (first, second) = tokio::join!(
        set_super_priority(&pool, &a.id, true),
        set_super_priority(&pool, &b.id, true)
    );
    first.unwrap();
    second.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE type='openai' AND provider='openai' AND super_priority=1").fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
    pool.close().await;
    let reopened = crate::database::connect(&path).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE super_priority=1")
        .fetch_one(&reopened)
        .await
        .unwrap();
    assert_eq!(count, 1);
    reopened.close().await;
    tokio::fs::remove_dir_all(folder).await.unwrap();
}
