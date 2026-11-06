pub mod account_controller;
pub mod anthropic_controller;
pub mod auth_controller;
pub mod middleware;
pub mod model_controller;
pub mod monitor_controller;
pub mod multiplier_monitor_controller;
pub mod openai_controller;
pub mod settings_controller;
pub mod statistics_controller;
pub mod usage_controller;
pub mod user_controller;

use std::{
    net::{AddrParseError, SocketAddr},
    sync::Arc,
};

use axum::{extract::DefaultBodyLimit, middleware::from_fn_with_state, routing::get, Router};
use tokio::net::TcpListener;
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
};

use crate::{app_state::ServerState, error::AppError};

const GRACEFUL_SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

const MAX_REQUEST_BODY_BYTES: usize = 512 * 1024 * 1024;

pub(crate) fn router(state: Arc<ServerState>) -> Router {
    let app: Router<Arc<ServerState>> = Router::new()
        .merge(openai_controller::routes())
        .merge(anthropic_controller::routes())
        .merge(account_controller::routes())
        .merge(model_controller::routes())
        .merge(usage_controller::routes())
        .merge(statistics_controller::routes())
        .merge(monitor_controller::routes())
        .merge(multiplier_monitor_controller::routes())
        .merge(settings_controller::routes())
        .merge(auth_controller::routes())
        .merge(user_controller::routes())
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"status":"ok"})) }),
        )
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .layer(from_fn_with_state(Arc::clone(&state), middleware::auth))
        .layer(CorsLayer::permissive());
    let web_root = std::env::var("AIMUX_WEB_ROOT").unwrap_or_else(|_| "dist".into());
    let index = format!("{web_root}/index.html");
    let app =
        app.fallback_service(ServeDir::new(web_root).not_found_service(ServeFile::new(index)));
    app.with_state(state)
}

pub async fn serve(state: Arc<ServerState>) -> Result<(), AppError> {
    let settings = state.runtime_settings.clone();
    let app = router(state);
    let addr: SocketAddr = format!("{}:{}", settings.host, settings.port)
        .parse::<SocketAddr>()
        .map_err(|error: AddrParseError| AppError::Internal(error.to_string()))?;
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    tracing::info!(%addr, "AIMux HTTP 服务已启动");
    serve_listener(listener, app, shutdown_signal()).await
}

async fn serve_listener<F>(listener: TcpListener, app: Router, shutdown: F) -> Result<(), AppError>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let server = axum::serve(listener, app).with_graceful_shutdown(shutdown);
    match tokio::time::timeout(GRACEFUL_SHUTDOWN_TIMEOUT, server).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(AppError::Internal(error.to_string())),
        Err(_) => Err(AppError::Internal(
            "服务在 30 秒排空期限内未能优雅关闭".into(),
        )),
    }
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut terminate = signal(SignalKind::terminate()).expect("注册 SIGTERM 处理器失败");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
    tracing::info!("收到关闭信号，停止接收新请求并等待现有请求完成");
}

#[cfg(test)]
mod multi_user_tests;

#[cfg(test)]
mod graceful_shutdown_tests {
    use super::*;
    use axum::{routing::get, Router};
    use std::time::Duration;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
    };

    #[tokio::test]
    async fn shutdown_waits_for_an_in_flight_request() {
        let app = Router::new().route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                "finished"
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server = tokio::spawn(serve_listener(listener, app, async move {
            let _ = shutdown_rx.await;
        }));

        let mut client = tokio::net::TcpStream::connect(address).await.unwrap();
        client
            .write_all(b"GET /slow HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        shutdown_tx.send(()).unwrap();

        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        let response = String::from_utf8(response).unwrap();
        assert!(response.contains("200 OK"));
        assert!(response.contains("finished"));
        server.await.unwrap().unwrap();
    }
}
