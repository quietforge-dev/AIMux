use aimux_lib::database::connect;
use sha2::{Digest, Sha384};
use sqlx::{
    raw_sql,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::path::Path;

fn checksum(source: &[u8]) -> Vec<u8> {
    Sha384::digest(source).to_vec()
}

fn normalize_line_endings(source: &[u8]) -> Vec<u8> {
    source
        .iter()
        .enumerate()
        .filter_map(|(index, byte)| {
            if *byte == b'\r' && source.get(index + 1) == Some(&b'\n') {
                None
            } else {
                Some(*byte)
            }
        })
        .collect()
}

fn crlf_line_endings(source: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(source.len());
    for byte in source {
        if *byte == b'\n' {
            result.push(b'\r');
        }
        result.push(*byte);
    }
    result
}

async fn create_legacy_database(path: &Path) -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .expect("创建旧版数据库失败");
    raw_sql(include_str!("../migrations/0001_baseline.sql"))
        .execute(&pool)
        .await
        .expect("创建旧版基线表失败");
    raw_sql(include_str!("../migrations/0003_models_one_default.sql"))
        .execute(&pool)
        .await
        .expect("执行旧版模型迁移失败");
    raw_sql(include_str!(
        "../migrations/0004_accounts_monitor_average_duration.sql"
    ))
    .execute(&pool)
    .await
    .expect("执行旧版账号迁移失败");
    raw_sql("CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY NOT NULL, description TEXT NOT NULL, installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, success BOOLEAN NOT NULL, checksum BLOB NOT NULL, execution_time BIGINT NOT NULL)")
        .execute(&pool)
        .await
        .expect("创建旧版迁移元数据失败");
    for (version, description, source) in [
        (
            1_i64,
            "baseline",
            include_bytes!("../migrations/0001_baseline.sql") as &[u8],
        ),
        (
            2_i64,
            "placeholder",
            include_bytes!("../migrations/0002_placeholder.sql") as &[u8],
        ),
        (
            3_i64,
            "models_one_default",
            include_bytes!("../migrations/0003_models_one_default.sql") as &[u8],
        ),
        (
            4_i64,
            "accounts_monitor_average_duration",
            include_bytes!("../migrations/0004_accounts_monitor_average_duration.sql") as &[u8],
        ),
    ] {
        sqlx::query("INSERT INTO _sqlx_migrations(version, description, success, checksum, execution_time) VALUES (?, ?, 1, ?, 0)")
            .bind(version)
            .bind(description)
            .bind(checksum(source))
            .execute(&pool)
            .await
            .expect("写入旧版迁移元数据失败");
    }
    pool
}

#[tokio::test]
async fn creates_and_reopens_baseline_database() {
    let path = std::env::temp_dir().join(format!("aimux-rust-{}.sqlite3", uuid::Uuid::new_v4()));
    let pool = connect(&path).await.expect("创建数据库失败");
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='accounts'",
    )
    .fetch_one(&pool)
    .await
    .expect("读取表失败");
    assert_eq!(count, 1);
    pool.close().await;
    let pool = connect(&path).await.expect("重新打开数据库失败");
    let migration: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE version=1 AND success=1")
            .fetch_one(&pool)
            .await
            .expect("读取迁移元数据失败");
    assert_eq!(migration, 1);
    pool.close().await;
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn accepts_migration_checksum_difference_caused_only_by_line_endings() {
    let path =
        std::env::temp_dir().join(format!("aimux-migration-{}.sqlite3", uuid::Uuid::new_v4()));
    let pool = connect(&path).await.expect("创建数据库失败");
    let source = include_bytes!("../migrations/0004_accounts_monitor_average_duration.sql");
    let lf_source = normalize_line_endings(source);
    let crlf_source = crlf_line_endings(&lf_source);
    let alternate_checksum = if checksum(source) == checksum(&lf_source) {
        checksum(&crlf_source)
    } else {
        checksum(&lf_source)
    };
    sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = 4")
        .bind(alternate_checksum)
        .execute(&pool)
        .await
        .expect("写入迁移校验和失败");
    pool.close().await;
    let reopened = connect(&path).await.expect("换行符兼容后打开数据库失败");
    let saved: Vec<u8> =
        sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version = 4")
            .fetch_one(&reopened)
            .await
            .expect("读取迁移校验和失败");
    assert_eq!(saved, checksum(source));
    reopened.close().await;
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn removes_legacy_multiplier_constraint_without_losing_accounts() {
    let path = std::env::temp_dir().join(format!(
        "aimux-multiplier-migration-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let legacy = create_legacy_database(&path).await;
    sqlx::query("INSERT INTO accounts (id, name, type, base_url, api_key_encrypted, status, priority, multiplier, total_requests, total_tokens, created_at, updated_at, monitor_average_duration_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind("legacy-account")
        .bind("旧账号")
        .bind("openai")
        .bind("https://example.test/v1")
        .bind("key")
        .bind("active")
        .bind(7_i64)
        .bind(0.30_f64)
        .bind(12_i64)
        .bind(34_i64)
        .bind("2026-01-01T00:00:00Z")
        .bind("2026-01-02T00:00:00Z")
        .bind(123_i64)
        .execute(&legacy)
        .await
        .expect("写入旧账号失败");
    legacy.close().await;

    let upgraded = connect(&path).await.expect("升级旧版数据库失败");
    let account: (String, f64, i64, i64, Option<i64>) = sqlx::query_as(
        "SELECT name, multiplier, total_requests, total_tokens, monitor_average_duration_ms FROM accounts WHERE id = 'legacy-account'",
    )
    .fetch_one(&upgraded)
    .await
    .expect("读取迁移后的账号失败");
    assert_eq!(account.0, "旧账号");
    assert_eq!(account.1, 0.30);
    assert_eq!(account.2, 12);
    assert_eq!(account.3, 34);
    assert_eq!(account.4, Some(123));

    sqlx::query("UPDATE accounts SET multiplier = 1.50 WHERE id = 'legacy-account'")
        .execute(&upgraded)
        .await
        .expect("倍率不应再受数据库 CHECK 约束限制");
    let multiplier: f64 =
        sqlx::query_scalar("SELECT multiplier FROM accounts WHERE id = 'legacy-account'")
            .fetch_one(&upgraded)
            .await
            .expect("读取更新后的倍率失败");
    assert_eq!(multiplier, 1.50);

    let schema: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'accounts'",
    )
    .fetch_one(&upgraded)
    .await
    .expect("读取 accounts 表结构失败");
    assert!(!schema.contains("ck_accounts_multiplier"));
    assert!(schema.contains("ck_accounts_priority"));
    upgraded.close().await;
    let _ = std::fs::remove_file(path);
}
