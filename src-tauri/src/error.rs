use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("请登录或检查访问令牌")]
    Unauthorized,
    #[error("没有此操作的权限")]
    Forbidden,
    #[error("登录尝试过于频繁，请稍后重试")]
    TooManyRequests,
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
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
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
