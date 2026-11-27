use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde_json::json;

use crate::db::DbError;

#[derive(Debug)]
pub enum ApiError {
    SignIn,
    NotFound,
    Forbidden,
    ForbiddenBecause(String),
    BadRequest(String),
    Conflict(String),
    TooLarge(String),
    Unsupported(String),
    TooMany(i64),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::SignIn => (StatusCode::UNAUTHORIZED, "sign in first".to_string()),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "not allowed".to_string()),
            ApiError::ForbiddenBecause(m) => (StatusCode::FORBIDDEN, m),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, m),
            ApiError::TooLarge(m) => (StatusCode::PAYLOAD_TOO_LARGE, m),
            ApiError::Unsupported(m) => (StatusCode::UNSUPPORTED_MEDIA_TYPE, m),
            ApiError::TooMany(wait) => {
                let mut res = (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({ "error": "too many requests for this token; wait a minute and try again" })),
                )
                    .into_response();
                if let Ok(v) = axum::http::HeaderValue::from_str(&wait.to_string()) {
                    res.headers_mut().insert(axum::http::header::RETRY_AFTER, v);
                }
                return res;
            }
            ApiError::Internal(m) => {
                eprintln!("tazko: internal error: {m}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "something went wrong".to_string(),
                )
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

impl From<DbError> for ApiError {
    fn from(e: DbError) -> Self {
        ApiError::Internal(e.to_string())
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}

pub fn bad(message: &str) -> ApiError {
    ApiError::BadRequest(message.to_string())
}

pub fn conflict(message: &str) -> ApiError {
    ApiError::Conflict(message.to_string())
}
