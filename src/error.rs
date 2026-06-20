use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;
use tracing::error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("配置错误：{0}")]
    Config(String),
    #[error("请求参数错误：{0}")]
    Validation(String),
    #[error("数据库暂不可用：{0}")]
    Database(String),
    #[error("外部模型服务错误：{0}")]
    Llm(String),
    #[error("资源不存在：{0}")]
    NotFound(String),
    #[error("服务器内部错误：{0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorBody {
    state: u16,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::Validation(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Database(_) | Self::Llm(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Config(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        if status.is_server_error() {
            error!(error = %self, "请求处理失败");
        }
        let body = ErrorBody {
            state: status.as_u16(),
            message: self.to_string(),
        };
        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        Self::Llm(value.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Internal(value.to_string())
    }
}
