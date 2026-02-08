use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use dallaspds_core::PdsError;
use serde_json::json;

#[derive(Debug)]
pub struct XrpcError {
    pub status: StatusCode,
    pub error_name: String,
    pub message: String,
}

impl XrpcError {
    pub fn new(status: StatusCode, error_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            error_name: error_name.into(),
            message: message.into(),
        }
    }
}

impl IntoResponse for XrpcError {
    fn into_response(self) -> Response {
        let body = json!({
            "error": self.error_name,
            "message": self.message,
        });
        (self.status, axum::Json(body)).into_response()
    }
}

impl From<PdsError> for XrpcError {
    fn from(err: PdsError) -> Self {
        match &err {
            PdsError::Storage(_) => XrpcError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalServerError",
                err.to_string(),
            ),
            PdsError::Crypto(_) => XrpcError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalServerError",
                err.to_string(),
            ),
            PdsError::Auth(_) => XrpcError::new(
                StatusCode::UNAUTHORIZED,
                "AuthenticationRequired",
                err.to_string(),
            ),
            PdsError::NotFound(_) => XrpcError::new(
                StatusCode::BAD_REQUEST,
                "NotFound",
                err.to_string(),
            ),
            PdsError::InvalidRequest(_) => XrpcError::new(
                StatusCode::BAD_REQUEST,
                "InvalidRequest",
                err.to_string(),
            ),
            PdsError::Upstream(_) => XrpcError::new(
                StatusCode::BAD_GATEWAY,
                "UpstreamFailure",
                err.to_string(),
            ),
            PdsError::AccountNotFound => XrpcError::new(
                StatusCode::BAD_REQUEST,
                "AccountNotFound",
                err.to_string(),
            ),
            PdsError::AccountTakendown => XrpcError::new(
                StatusCode::BAD_REQUEST,
                "AccountTakendown",
                err.to_string(),
            ),
            PdsError::AccountDeactivated => XrpcError::new(
                StatusCode::BAD_REQUEST,
                "AccountDeactivated",
                err.to_string(),
            ),
            PdsError::HandleAlreadyTaken => XrpcError::new(
                StatusCode::BAD_REQUEST,
                "HandleAlreadyTaken",
                err.to_string(),
            ),
            PdsError::InvalidHandle => XrpcError::new(
                StatusCode::BAD_REQUEST,
                "InvalidHandle",
                err.to_string(),
            ),
            PdsError::InvalidPassword => XrpcError::new(
                StatusCode::UNAUTHORIZED,
                "InvalidPassword",
                err.to_string(),
            ),
            PdsError::SessionExpired => XrpcError::new(
                StatusCode::UNAUTHORIZED,
                "ExpiredToken",
                err.to_string(),
            ),
            PdsError::InvalidInviteCode => XrpcError::new(
                StatusCode::BAD_REQUEST,
                "InvalidInviteCode",
                err.to_string(),
            ),
            PdsError::InviteCodeExhausted => XrpcError::new(
                StatusCode::BAD_REQUEST,
                "InvalidInviteCode",
                "Invite code has no remaining uses",
            ),
            PdsError::Forbidden(_) => XrpcError::new(
                StatusCode::FORBIDDEN,
                "AuthorizationError",
                err.to_string(),
            ),
            PdsError::InternalError(_) => XrpcError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "InternalServerError",
                err.to_string(),
            ),
        }
    }
}
