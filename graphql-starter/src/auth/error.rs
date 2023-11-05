use error_info::ErrorInfo;
use http::StatusCode;

/// Authentication and authorization related errors
#[derive(Debug, ErrorInfo)]
#[allow(clippy::enum_variant_names)]
pub enum AuthErrorCode {
    #[error(status = StatusCode::UNAUTHORIZED, message = "Missing authentication")]
    AuthMissing,
    #[error(status = StatusCode::BAD_REQUEST, message = "Malformed cookies")]
    AuthMalformedCookies,
    #[error(status = StatusCode::BAD_REQUEST, message = "Malformed \"{auth_header}\" header")]
    AuthMalformedAuthHeader { auth_header: String },
    #[error(status = StatusCode::BAD_REQUEST, message = "Invalid authorization token")]
    AuthInvalidToken,
    #[error(status = StatusCode::FORBIDDEN, message = "The user is not allowed to perform such action")]
    AuthFailed,
}
