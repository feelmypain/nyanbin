use axum::{
    Json,
    http::{HeaderValue, StatusCode, header::RETRY_AFTER},
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: &'static str,
    pub retry_after: Option<u64>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct ErrorBody {
    code: &'static str,
    message: &'static str,
}

impl ApiError {
    pub const fn new(status: StatusCode, code: &'static str, message: &'static str) -> Self {
        Self {
            status,
            code,
            message,
            retry_after: None,
        }
    }

    pub const fn invalid_request(message: &'static str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", message)
    }

    pub const fn storage() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "storage_unavailable",
            "Storage is temporarily unavailable",
        )
    }

    pub const fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "note_not_found", "Note not found")
    }

    /// 429 with a mandatory Retry-After header (clamped to at least 1s).
    pub const fn rate_limited(retry_after: u64) -> Self {
        let mut error = Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many requests for this operation; try again later",
        );
        error.retry_after = Some(if retry_after == 0 { 1 } else { retry_after });
        error
    }

    pub const fn writes_disabled() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "writes_disabled",
            "Note writes are temporarily paused by the operator",
        )
    }

    pub const fn short_disabled() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "short_disabled",
            "Short links are temporarily paused by the operator",
        )
    }

    pub const fn storage_pressure() -> Self {
        Self::new(
            StatusCode::INSUFFICIENT_STORAGE,
            "storage_pressure",
            "Storage is under pressure; retry later or with a smaller note",
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(ErrorBody {
                code: self.code,
                message: self.message,
            }),
        )
            .into_response();
        if let Some(seconds) = self.retry_after {
            if let Ok(value) = HeaderValue::from_str(&seconds.max(1).to_string()) {
                response.headers_mut().insert(RETRY_AFTER, value);
            }
        }
        response
    }
}

pub fn json_rejection(rejection: axum::extract::rejection::JsonRejection) -> ApiError {
    if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            "Request body is too large",
        )
    } else {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_json",
            "Request body must be valid JSON matching the API schema",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limited_always_carries_positive_retry_after() {
        assert_eq!(ApiError::rate_limited(0).retry_after, Some(1));
        assert_eq!(ApiError::rate_limited(42).retry_after, Some(42));
        let response = ApiError::rate_limited(0).into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response
                .headers()
                .get(RETRY_AFTER)
                .unwrap()
                .to_str()
                .unwrap(),
            "1"
        );
    }

    #[test]
    fn plain_errors_do_not_emit_retry_after() {
        let response = ApiError::not_found().into_response();
        assert!(response.headers().get(RETRY_AFTER).is_none());
    }

    #[test]
    fn kill_switch_and_pressure_codes_match_contract() {
        assert_eq!(
            ApiError::writes_disabled().status,
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(ApiError::writes_disabled().code, "writes_disabled");
        assert_eq!(ApiError::short_disabled().code, "short_disabled");
        assert_eq!(
            ApiError::storage_pressure().status,
            StatusCode::INSUFFICIENT_STORAGE
        );
        assert_eq!(ApiError::storage_pressure().code, "storage_pressure");
    }
}
