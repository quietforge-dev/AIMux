use super::*;
use crate::{
    config::Settings,
    dao::{
        account_dao, daily_statistics_dao, model_dao, monitor_dao, multiplier_monitor_dao,
        settings_dao, usage_dao, user_dao,
    },
    database::UserDb,
    service::{auth_service, settings_service},
};
use axum::Json;
use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn setup() -> Arc<ServerState> {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let settings = Settings {
        upstream_proxy_enabled: false,
        low_cache_min_input_tokens: 4096,
        ..Settings::default()
    };
    let hash = auth_service::hash_password("owner-password").await.unwrap();
    user_dao::initialize_owner(
        &pool,
        "owner",
        &hash,
        Some((&auth_service::token_hash("legacy-token"), "legacy-token")),
        &settings,
    )
    .await
    .unwrap();
    ServerState::testing(pool, settings, false)
}
async fn call(
    s: &Arc<ServerState>,
    method: Method,
    path: &str,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = token {
        req = req.header("authorization", format!("Bearer {token}"));
    }
    let req = req
        .body(if body.is_null() {
            Body::empty()
        } else {
            Body::from(body.to_string())
        })
        .unwrap();
    let response = router(s.clone()).oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 2 * 1024 * 1024)
        .await
        .unwrap();
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| json!({"text":String::from_utf8_lossy(&bytes)}));
    (status, value)
}
async fn login(s: &Arc<ServerState>, name: &str, password: &str) -> String {
    let (status, result) = call(
        s,
        Method::POST,
        "/api/auth/login",
        None,
        json!({"username":name,"password":password}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    result["token"].as_str().unwrap().to_owned()
}
async fn colleague(s: &Arc<ServerState>) -> (crate::model::user::User, String) {
    auth_service::create_user(&s.pool, "colleague", "colleague-password")
        .await
        .unwrap()
}
async fn create_account(db: &UserDb, name: &str) -> crate::model::account::Account {
    account_dao::create(db,serde_json::from_value(json!({"name":name,"type":"openai","base_url":"http://127.0.0.1:1/v1","api_key":"fixture-secret","provider":"openai","priority":5,"multiplier":0.1})).unwrap()).await.unwrap()
}
fn usage(id: &str, account: &str, tokens: i64) -> crate::model::usage_record::UsageRecord {
    serde_json::from_value(json!({"id":id,"trace_id":id,"started_at":"2026-10-20T08:00:00Z","ended_at":"2026-10-20T08:00:01Z","duration_ms":1000,"first_token_ms":100,"account_id":account,"account_name":"fixture","account_type":"openai","model":"same-model","reasoning_effort":"high","stream":false,"success":true,"input_tokens":tokens,"output_tokens":10,"cached_tokens":0,"total_tokens":tokens+10,"attempts":1})).unwrap()
}

#[tokio::test]
async fn web_sessions_roles_token_rotation_disable_logout_and_restart() {
    let s = setup().await;
    assert_eq!(
        call(&s, Method::GET, "/api/accounts", None, Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let owner = login(&s, "owner", "owner-password").await;
    let (status, created) = call(
        &s,
        Method::POST,
        "/api/users",
        Some(&owner),
        json!({"username":"colleague","password":"colleague-password"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let id = created["user"]["id"].as_str().unwrap();
    let api_token = created["token"].as_str().unwrap();
    assert!(created["user"].get("password_hash").is_none());
    assert!(created["user"].get("api_token_hash").is_none());
    let user_session = login(&s, "colleague", "colleague-password").await;
    assert_eq!(
        call(
            &s,
            Method::GET,
            "/api/users",
            Some(&user_session),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &s,
            Method::POST,
            "/api/users",
            Some(&user_session),
            json!({"username":"evil","password":"password"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &s,
            Method::GET,
            "/api/accounts",
            Some(api_token),
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &s,
            Method::GET,
            "/v1/models",
            Some(&user_session),
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&s, Method::GET, "/v1/models", Some(api_token), Value::Null)
            .await
            .0,
        StatusCode::OK
    );
    let restarted = ServerState::testing(s.pool.clone(), s.runtime_settings.clone(), false);
    assert_eq!(
        call(
            &restarted,
            Method::GET,
            "/api/auth/me",
            Some(&user_session),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, rotated) = call(
        &s,
        Method::POST,
        "/api/auth/token/reset",
        Some(&user_session),
        Value::Null,
    )
    .await;
    let next = rotated["token"].as_str().unwrap();
    assert_eq!(
        call(&s, Method::GET, "/v1/models", Some(api_token), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&s, Method::GET, "/v1/models", Some(next), Value::Null)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &s,
            Method::POST,
            "/api/users/local/enabled",
            Some(&owner),
            json!({"enabled":false})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &s,
            Method::POST,
            &format!("/api/users/{id}/enabled"),
            Some(&owner),
            json!({"enabled":false})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&s, Method::GET, "/v1/models", Some(next), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &s,
            Method::GET,
            "/api/auth/me",
            Some(&user_session),
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    call(
        &s,
        Method::POST,
        &format!("/api/users/{id}/enabled"),
        Some(&owner),
        json!({"enabled":true}),
    )
    .await;
    assert_eq!(
        call(
            &s,
            Method::GET,
            "/api/auth/me",
            Some(&user_session),
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let fresh = login(&s, "colleague", "colleague-password").await;
    call(
        &s,
        Method::POST,
        "/api/auth/logout",
        Some(&fresh),
        Value::Null,
    )
    .await;
    assert_eq!(
        call(&s, Method::GET, "/api/auth/me", Some(&fresh), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &s,
            Method::GET,
            "/v1/models",
            Some("legacy-token"),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn business_reads_writes_models_settings_monitors_and_cleanup_are_scoped() {
    let s = setup().await;
    let (user, _) = colleague(&s).await;
    let owner = user_dao::get(&s.pool, "local").await.unwrap();
    let a = s.user_state(&owner).await.unwrap();
    let b = s.user_state(&user).await.unwrap();
    let aa = create_account(&a.pool, "same-name").await;
    let bb = create_account(&b.pool, "same-name").await;
    for db in [&a.pool, &b.pool] {
        let model = model_dao::create(
            db,
            serde_json::from_value(
                json!({"name":"same-model","type":"openai","provider":"openai"}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
        model_dao::set_default(db, model).await.unwrap();
        assert_eq!(
            model_dao::default_name(db, "openai")
                .await
                .unwrap()
                .as_deref(),
            Some("same-model")
        );
    }
    assert!(account_dao::get(&b.pool, &aa.id).await.unwrap().is_none());
    account_dao::save_priority(&b.pool, &aa.id, 0)
        .await
        .unwrap();
    account_dao::delete(&b.pool, &aa.id).await.unwrap();
    assert_eq!(
        account_dao::get(&a.pool, &aa.id)
            .await
            .unwrap()
            .unwrap()
            .priority,
        5
    );
    assert_eq!(
        account_dao::pick_one(&b.pool, Some("same-model"), "openai")
            .await
            .unwrap()
            .unwrap()
            .id,
        bb.id
    );
    usage_dao::create(&a.pool, &usage("a-usage", &aa.id, 100))
        .await
        .unwrap();
    usage_dao::create(&b.pool, &usage("b-usage", &bb.id, 900))
        .await
        .unwrap();
    assert!(usage_dao::get(&b.pool, "a-usage").await.unwrap().is_none());
    assert!(usage_dao::create(&b.pool, &usage("forged", &aa.id, 1))
        .await
        .is_err());
    let record = crate::model::monitor_record::MonitorRecord {
        id: "monitor-a".into(),
        account_id: aa.id.clone(),
        account_name: "A".into(),
        account_type: "openai".into(),
        model: None,
        checked_at: "2026-10-20".into(),
        duration_ms: Some(50),
        success: true,
        status_code: Some(200),
        error_code: None,
        error_message: None,
    };
    monitor_dao::create_and_refresh_account_average(&a.pool, &record)
        .await
        .unwrap();
    assert!(monitor_dao::list_grouped(&b.pool, &[aa.id.clone()], 20)
        .await
        .unwrap()
        .is_empty());
    assert!(multiplier_monitor_dao::upsert_config(
        &b.pool, &aa.id, "sub2api", None, false, 1, true, "now"
    )
    .await
    .is_err());
    multiplier_monitor_dao::upsert_config(&a.pool, &aa.id, "sub2api", None, false, 1, true, "now")
        .await
        .unwrap();
    assert!(multiplier_monitor_dao::get_config(&b.pool, &aa.id)
        .await
        .unwrap()
        .is_none());
    assert!(multiplier_monitor_dao::claim_config(
        &b.pool, &aa.id, "B", "now", "later", "now", true
    )
    .await
    .unwrap()
    .is_none());
    let mut custom = b.settings.read().await.clone();
    custom.upstream_proxy_enabled = true;
    custom.upstream_proxy_url = "http://127.0.0.1:9999".into();
    custom.low_cache_min_input_tokens = 8192;
    settings_service::update(&b.pool, false, &b.settings, custom)
        .await
        .unwrap();
    assert!(!a.settings.read().await.upstream_proxy_enabled);
    assert_eq!(a.settings.read().await.low_cache_min_input_tokens, 4096);
    assert_eq!(
        settings_dao::load(&b.pool, &s.runtime_settings)
            .await
            .unwrap()
            .low_cache_min_input_tokens,
        8192
    );
    for (db, expected) in [(&a.pool, 110), (&b.pool, 910)] {
        daily_statistics_dao::archive_day(
            db,
            "2026-10-20",
            "2026-10-20T00:00:00Z",
            "2026-10-21T00:00:00Z",
            "now",
        )
        .await
        .unwrap();
        assert_eq!(daily_statistics_dao::total(db).await.unwrap().3, expected);
    }
    // A late record must update only the owner's archived totals.
    usage_dao::create(&a.pool, &usage("late", &aa.id, 50))
        .await
        .unwrap();
    assert_eq!(daily_statistics_dao::total(&a.pool).await.unwrap().3, 170);
    assert_eq!(daily_statistics_dao::total(&b.pool).await.unwrap().3, 910);
    assert_eq!(
        usage_dao::cleanup(&b.pool, "2026-10-21T00:00:00Z")
            .await
            .unwrap(),
        1
    );
    assert!(usage_dao::get(&a.pool, "a-usage").await.unwrap().is_some());
    assert_eq!(daily_statistics_dao::total(&b.pool).await.unwrap().3, 910);
    let owner_session = login(&s, "owner", "owner-password").await;
    let session = login(&s, "colleague", "colleague-password").await;
    for suffix in ["", "/test", "/super-priority", "/set-status"] {
        let path = format!("/api/accounts/{}{suffix}", aa.id);
        let method = if suffix.is_empty() {
            Method::GET
        } else {
            Method::POST
        };
        let body = match suffix {
            "/test" => json!({}),
            "/super-priority" => json!({"enabled":true}),
            "/set-status" => json!({"status":"disabled"}),
            _ => Value::Null,
        };
        assert_eq!(
            call(&s, method, &path, Some(&session), body).await.0,
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
    let (_, listed) = call(
        &s,
        Method::GET,
        "/api/accounts",
        Some(&owner_session),
        Value::Null,
    )
    .await;
    assert_eq!(listed["total"], 1);
    assert_eq!(listed["items"][0]["id"], aa.id);
}

#[tokio::test]
async fn password_reset_expiry_and_admin_invariants() {
    let s = setup().await;
    let (user, api_token) = colleague(&s).await;
    let session = login(&s, "colleague", "colleague-password").await;
    let wrong = call(
        &s,
        Method::POST,
        "/api/auth/password",
        Some(&session),
        json!({"current_password":"wrong","new_password":"new-password"}),
    )
    .await;
    assert_eq!(wrong.0, StatusCode::BAD_REQUEST);
    assert_eq!(
        call(
            &s,
            Method::POST,
            "/api/auth/password",
            Some(&session),
            json!({"current_password":"colleague-password","new_password":"new-password"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&s, Method::GET, "/api/auth/me", Some(&session), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&s, Method::GET, "/v1/models", Some(&api_token), Value::Null)
            .await
            .0,
        StatusCode::OK
    );
    let session = login(&s, "colleague", "new-password").await;
    sqlx::query("UPDATE web_sessions SET expires_at=0 WHERE user_id=?")
        .bind(&user.id)
        .execute(&s.pool)
        .await
        .unwrap();
    assert_eq!(
        call(&s, Method::GET, "/api/auth/me", Some(&session), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    for statement in [
        "DELETE FROM users WHERE id='local'",
        "UPDATE users SET role='user' WHERE id='local'",
        "UPDATE users SET enabled=0 WHERE id='local'",
    ] {
        assert!(sqlx::query(statement).execute(&s.pool).await.is_err());
    }
    assert!(sqlx::query("UPDATE users SET role='admin' WHERE id=?")
        .bind(&user.id)
        .execute(&s.pool)
        .await
        .is_err());
    assert!(auth_service::create_user(&s.pool, "COLLEAGUE", "password")
        .await
        .is_err());
}

#[tokio::test]
async fn initial_import_is_transactional_and_never_overwrites_changed_credentials_or_settings() {
    let s = setup().await;
    let db = UserDb::new(s.pool.clone(), "local");
    let mut changed = settings_dao::load(&db, &s.runtime_settings).await.unwrap();
    changed.low_cache_min_input_tokens = 16384;
    settings_dao::save(&db, &changed).await.unwrap();
    user_dao::initialize_owner(&s.pool, "replaced", "bad-hash", None, &Settings::default())
        .await
        .unwrap();
    assert_eq!(
        user_dao::get(&s.pool, "local").await.unwrap().username,
        "owner"
    );
    assert_eq!(
        settings_dao::load(&db, &Settings::default())
            .await
            .unwrap()
            .low_cache_min_input_tokens,
        16384
    );
    let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM user_settings WHERE key IN ('host','port','local_token','launch_at_login')").fetch_one(&s.pool).await.unwrap();
    assert_eq!(count, 0);
    sqlx::query("UPDATE user_settings SET value_json='\"wrong-type\"' WHERE user_id='local' AND key='monitoring_enabled'").execute(&s.pool).await.unwrap();
    assert!(settings_dao::load(&db, &Settings::default()).await.is_err());
}

#[tokio::test]
async fn desktop_uses_internal_identity_without_web_user_management() {
    let web = setup().await;
    let s = ServerState::testing(web.pool.clone(), Settings::default(), true);
    assert_eq!(
        call(&s, Method::GET, "/api/auth/me", None, Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let owner = user_dao::get(&s.pool, "local").await.unwrap();
    let state = s.user_state(&owner).await.unwrap();
    let secret = state.desktop_secret.as_deref().unwrap();
    let (status, identity) = call(&s, Method::GET, "/api/auth/me", Some(secret), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(identity["desktop"], true);
    assert_eq!(
        call(&s, Method::GET, "/api/users", Some(secret), Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &s,
            Method::POST,
            "/api/auth/login",
            None,
            json!({"username":"owner","password":"owner-password"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (_, rotated) = call(
        &s,
        Method::POST,
        "/api/auth/token/reset",
        Some(secret),
        Value::Null,
    )
    .await;
    assert_eq!(
        call(
            &s,
            Method::GET,
            "/v1/models",
            Some("legacy-token"),
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &s,
            Method::GET,
            "/v1/models",
            rotated["token"].as_str(),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&s, Method::GET, "/api/settings", Some(secret), Value::Null)
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn gateway_tokens_route_and_record_nonstream_and_stream_for_their_owners() {
    use axum::{response::IntoResponse, routing::post};
    async fn upstream_a(Json(body): Json<Value>) -> axum::response::Response {
        upstream("A", body)
    }
    async fn upstream_b(Json(body): Json<Value>) -> axum::response::Response {
        upstream("B", body)
    }
    fn upstream(owner: &str, body: Value) -> axum::response::Response {
        if body["stream"] == true {
            let chunk = json!({"id":"fixture","choices":[{"index":0,"delta":{"content":owner},"finish_reason":null}]});
            let done = json!({"id":"fixture","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":3000,"completion_tokens":10,"total_tokens":3010}});
            (
                [("content-type", "text/event-stream")],
                format!("data: {chunk}\n\ndata: {done}\n\ndata: [DONE]\n\n"),
            )
                .into_response()
        } else {
            Json(json!({"id":"fixture","choices":[{"message":{"role":"assistant","content":owner},"finish_reason":"stop"}],"usage":{"prompt_tokens":3000,"completion_tokens":10,"total_tokens":3010}})).into_response()
        }
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let upstream_task = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route("/a/v1/chat/completions", post(upstream_a))
                .route("/b/v1/chat/completions", post(upstream_b)),
        )
        .await
        .unwrap();
    });
    let s = setup().await;
    let (user, token) = colleague(&s).await;
    let owner = user_dao::get(&s.pool, "local").await.unwrap();
    let a = s.user_state(&owner).await.unwrap();
    let b = s.user_state(&user).await.unwrap();
    for (db, path) in [(&a.pool, "a"), (&b.pool, "b")] {
        account_dao::create(db,serde_json::from_value(json!({"name":"own-upstream","type":"openai","base_url":format!("{origin}/{path}/v1"),"api_key":"fake-key"})).unwrap()).await.unwrap();
    }
    for stream in [false, true] {
        let body = json!({"model":"same-model","messages":[{"role":"user","content":"test"}],"stream":stream,"reasoning_effort":"high"});
        let ((sa, ra), (sb, rb)) = tokio::join!(
            call(
                &s,
                Method::POST,
                "/v1/chat/completions",
                Some("legacy-token"),
                body.clone()
            ),
            call(&s, Method::POST, "/v1/chat/completions", Some(&token), body)
        );
        assert_eq!(sa, StatusCode::OK, "{ra}");
        assert_eq!(sb, StatusCode::OK, "{rb}");
        if stream {
            assert!(ra["text"].as_str().unwrap().contains("\"content\":\"A\""));
            assert!(rb["text"].as_str().unwrap().contains("\"content\":\"B\""));
        } else {
            assert_eq!(ra["choices"][0]["message"]["content"], "A");
            assert_eq!(rb["choices"][0]["message"]["content"], "B");
        }
    }
    for db in [&a.pool, &b.pool] {
        let (rows, total) = usage_dao::list(db, 0, 20, None, None, None, None, None, None, None)
            .await
            .unwrap();
        assert_eq!(total, 2);
        assert!(rows.iter().all(|r| r.success && r.ended_at.is_some()));
        assert_eq!(daily_statistics_dao::total(db).await.unwrap().3, 6020);
        assert_eq!(
            account_dao::list(db, 0, 20, None, None, None)
                .await
                .unwrap()
                .0[0]
                .total_requests,
            2
        );
    }
    upstream_task.abort();
}

#[tokio::test]
async fn failed_settings_transaction_preserves_database_and_runtime() {
    let s = setup().await;
    let owner = user_dao::get(&s.pool, "local").await.unwrap();
    let a = s.user_state(&owner).await.unwrap();
    let before = a.settings.read().await.clone();
    sqlx::query("CREATE TRIGGER reject_setting BEFORE INSERT ON user_settings WHEN NEW.key='low_cache_min_input_tokens' AND NEW.value_json='9999' BEGIN SELECT RAISE(ABORT,'fixture failure'); END").execute(&s.pool).await.unwrap();
    let mut changed = before.clone();
    changed.first_token_timeout_seconds = 7;
    changed.low_cache_min_input_tokens = 9999;
    assert!(
        settings_service::update(&a.pool, false, &a.settings, changed)
            .await
            .is_err()
    );
    assert_eq!(
        a.settings.read().await.first_token_timeout_seconds,
        before.first_token_timeout_seconds
    );
    let loaded = settings_dao::load(&a.pool, &s.runtime_settings)
        .await
        .unwrap();
    assert_eq!(
        loaded.first_token_timeout_seconds,
        before.first_token_timeout_seconds
    );
    assert_eq!(
        loaded.low_cache_min_input_tokens,
        before.low_cache_min_input_tokens
    );
}

#[tokio::test]
async fn login_attempts_are_bounded_and_extra_role_fields_cannot_create_an_admin() {
    let s = setup().await;
    for _ in 0..10 {
        assert_eq!(
            call(
                &s,
                Method::POST,
                "/api/auth/login",
                None,
                json!({"username":"unknown","password":"wrong"})
            )
            .await
            .0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        call(
            &s,
            Method::POST,
            "/api/auth/login",
            None,
            json!({"username":"unknown","password":"wrong"})
        )
        .await
        .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    let owner = login(&s, "owner", "owner-password").await;
    let (status, result) = call(
        &s,
        Method::POST,
        "/api/users",
        Some(&owner),
        json!({"username":"ordinary","password":"password","role":"admin","user_id":"local"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["user"]["role"], "user");
    let session = login(&s, "ordinary", "password").await;
    let (status,account)=call(&s,Method::POST,"/api/accounts",Some(&session),json!({"name":"injected-owner","type":"openai","base_url":"http://127.0.0.1:1/v1","api_key":"fixture-key","user_id":"local"})).await;
    assert_eq!(status, StatusCode::OK, "{account}");
    let owner_db = UserDb::new(s.pool.clone(), "local");
    assert!(account_dao::get(&owner_db, account["id"].as_str().unwrap())
        .await
        .unwrap()
        .is_none());
    let (_, list) = call(
        &s,
        Method::GET,
        "/api/accounts?user_id=local",
        Some(&session),
        Value::Null,
    )
    .await;
    assert_eq!(list["total"], 1);
    assert_eq!(list["items"][0]["id"], account["id"]);
}

/// Explicitly opt in to a disposable in-memory server for browser acceptance tests.
#[tokio::test]
#[ignore = "manual browser fixture; serves only an in-memory test database"]
async fn browser_fixture() {
    let s = setup().await;
    let mut settings = s.runtime_settings.clone();
    settings.port = 17889;
    let fixture = ServerState::testing(s.pool.clone(), settings, false);
    serve(fixture).await.unwrap();
}
