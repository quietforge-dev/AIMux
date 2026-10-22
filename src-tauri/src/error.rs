use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),
    #[error("数据库迁移错误: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("{0}")]
    BadRequest(String),
    #[error("资源不存在: {0}")]
    NotFound(String),
    #[error("上游请求失败: {0}")]
    Upstream(String),
    #[error("上游请求超时: {0}")]
    UpstreamTimeout(String),
    #[error("内部错误: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self {
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Upstream(_) => StatusCode::BAD_GATEWAY,
            AppError::UpstreamTimeout(_) => StatusCode::GATEWAY_TIMEOUT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (
            status,
            axum::Json(serde_json::json!({"detail": self.to_string()})),
        )
            .into_response()
    }
}
