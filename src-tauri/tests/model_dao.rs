use aimux_lib::{
    dao::model_dao::{create, get, list, set_default, update},
    database::connect,
    schema::model_schema::{ModelCreate, ModelUpdate},
};

#[tokio::test]
async fn switches_the_only_default_model_in_a_transaction() {
    let path = std::env::temp_dir().join(format!(
        "aimux-model-default-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let pool = connect(&path).await.expect("创建数据库失败");
    let first = create(
        &pool,
        ModelCreate {
            name: "model-a".into(),
            model_type: "openai".into(),
            provider: "openai".into(),
        },
    )
    .await
    .expect("创建第一个模型失败");
    let second = create(
        &pool,
        ModelCreate {
            name: "model-b".into(),
            model_type: "openai".into(),
            provider: "deepseek".into(),
        },
    )
    .await
    .expect("创建第二个模型失败");
    set_default(&pool, first)
        .await
        .expect("设置第一个默认模型失败");
    let duplicate_default = sqlx::query("UPDATE models SET is_default=1 WHERE id=?")
        .bind(&second.id)
        .execute(&pool)
        .await;
    assert!(duplicate_default.is_err());
    set_default(&pool, second.clone())
        .await
        .expect("切换默认模型失败");
    let defaults: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM models WHERE type='openai' AND is_default=1")
            .fetch_one(&pool)
            .await
            .expect("读取默认模型数量失败");
    assert_eq!(defaults, 1);
    assert_eq!(
        get(&pool, &second.id)
            .await
            .expect("读取第二个模型失败")
            .expect("第二个模型不存在")
            .is_default,
        1
    );
    pool.close().await;
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn filters_and_updates_model_provider_without_changing_default_grouping() {
    let path = std::env::temp_dir().join(format!(
        "aimux-model-provider-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let pool = connect(&path).await.expect("创建数据库失败");
    let openai = create(
        &pool,
        ModelCreate {
            name: "provider-openai".into(),
            model_type: "openai".into(),
            provider: "openai".into(),
        },
    )
    .await
    .expect("创建 OpenAI 模型失败");
    let deepseek = create(
        &pool,
        ModelCreate {
            name: "provider-deepseek".into(),
            model_type: "openai".into(),
            provider: "deepseek".into(),
        },
    )
    .await
    .expect("创建 DeepSeek 模型失败");
    set_default(&pool, openai.clone())
        .await
        .expect("设置默认模型失败");

    let filtered = list(&pool, Some("openai"), Some("deepseek"))
        .await
        .expect("按供应商筛选失败");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, deepseek.id);

    let updated = update(
        &pool,
        deepseek,
        ModelUpdate {
            provider: Some("custom-relay".into()),
            ..Default::default()
        },
    )
    .await
    .expect("更新非预置供应商失败");
    assert_eq!(updated.provider, "custom-relay");
    assert_eq!(
        get(&pool, &openai.id)
            .await
            .expect("读取默认模型失败")
            .expect("默认模型不存在")
            .is_default,
        1
    );

    pool.close().await;
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn lists_models_by_type_provider_default_and_name() {
    let path = std::env::temp_dir().join(format!(
        "aimux-model-order-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let pool = connect(&path).await.expect("创建数据库失败");

    create(
        &pool,
        ModelCreate {
            name: "claude-z".into(),
            model_type: "anthropic".into(),
            provider: "anthropic".into(),
        },
    )
    .await
    .expect("创建 Anthropic 模型失败");
    create(
        &pool,
        ModelCreate {
            name: "deepseek-z".into(),
            model_type: "openai".into(),
            provider: "deepseek".into(),
        },
    )
    .await
    .expect("创建 DeepSeek Z 模型失败");
    create(
        &pool,
        ModelCreate {
            name: "deepseek-a".into(),
            model_type: "openai".into(),
            provider: "deepseek".into(),
        },
    )
    .await
    .expect("创建 DeepSeek A 模型失败");
    let openai_default = create(
        &pool,
        ModelCreate {
            name: "gpt-default".into(),
            model_type: "openai".into(),
            provider: "openai".into(),
        },
    )
    .await
    .expect("创建 OpenAI 默认模型失败");
    set_default(&pool, openai_default)
        .await
        .expect("设置 OpenAI 默认模型失败");

    let listed = list(&pool, None, None).await.expect("查询模型列表失败");
    assert_eq!(
        listed
            .iter()
            .map(|model| model.name.as_str())
            .collect::<Vec<_>>(),
        ["claude-z", "deepseek-a", "deepseek-z", "gpt-default"]
    );

    pool.close().await;
    let _ = std::fs::remove_file(path);
}
