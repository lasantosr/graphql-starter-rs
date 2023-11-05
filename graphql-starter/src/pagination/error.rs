use error_info::ErrorInfo;
use http::StatusCode;

/// Error codes when using pagination
#[derive(Debug, ErrorInfo)]
#[allow(clippy::enum_variant_names)]
pub enum PaginationErrorCode {
    #[error(status=StatusCode::BAD_REQUEST, message = "Missing pagination data: at least one of \"first\" or \"last\" must be set")]
    PageMissing,
    #[error(status=StatusCode::BAD_REQUEST, message = "The \"{field}\" parameter must be a non-negative number")]
    PageNegativeInput { field: &'static str },
    #[error(status=StatusCode::BAD_REQUEST, message = "The \"first\" and \"last\" parameters cannot exist at the same time")]
    PageFirstAndLast,
    #[error(status=StatusCode::BAD_REQUEST, message = "The \"after\" and \"before\" parameters cannot exist at the same time")]
    PageAfterAndBefore,
    #[error(status=StatusCode::BAD_REQUEST, message = "When forward paginating only \"after\" is allowed, not \"before\"")]
    PageForwardWithBefore,
    #[error(status=StatusCode::BAD_REQUEST, message = "When backward paginating only \"before\" is allowed, not \"after\"")]
    PageBackwardWithAfter,
    #[error(status=StatusCode::BAD_REQUEST, message = "The provided cursor is not recognized")]
    PageInvalidCursor,
}
