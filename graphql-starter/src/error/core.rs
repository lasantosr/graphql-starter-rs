use std::{collections::HashMap, fmt, sync::Arc};

use error_info::ErrorInfo;
use http::StatusCode;
use tracing_error::SpanTrace;

pub type Result<T, E = Box<Error>> = std::result::Result<T, E>;

/// Generic error codes, they're usually not meant for the end-user
#[derive(Clone, Copy, ErrorInfo)]
pub enum GenericErrorCode {
    #[error(status = StatusCode::BAD_REQUEST, message = "The request is not well formed")]
    BadRequest,
    #[error(status = StatusCode::UNAUTHORIZED, message = "Not authorized to access this resource")]
    Unauthorized,
    #[error(status = StatusCode::FORBIDDEN, message = "Forbidden access to the resource")]
    Forbidden,
    #[error(status = StatusCode::NOT_FOUND, message = "The resource could not be found")]
    NotFound,
    #[error(status = StatusCode::GATEWAY_TIMEOUT, message = "Timeout exceeded while waiting for a response")]
    GatewayTimeout,
    #[error(status = StatusCode::INTERNAL_SERVER_ERROR, message = "Internal server error")]
    InternalServerError,
}

/// This type represents an error in the service
#[derive(Clone)]
pub struct Error {
    pub(super) info: Arc<dyn ErrorInfo + Send + Sync + 'static>,
    pub(super) reason: Option<String>,
    pub(super) properties: Option<HashMap<String, serde_json::Value>>,
    pub(super) unexpected: bool,
    pub(super) source: Option<Arc<dyn fmt::Display + Send + Sync>>,
    pub(super) context: SpanTrace,
}
struct ErrorInfoDebug {
    status: StatusCode,
    code: &'static str,
    raw_message: &'static str,
    fields: HashMap<String, String>,
}
impl fmt::Debug for ErrorInfoDebug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErrorInfo")
            .field("status", &self.status)
            .field("code", &self.code)
            .field("raw_message", &self.raw_message)
            .field("fields", &self.fields)
            .finish()
    }
}
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field(
                "info",
                &ErrorInfoDebug {
                    status: self.info.status(),
                    code: self.info.code(),
                    raw_message: self.info.raw_message(),
                    fields: self.info.fields(),
                },
            )
            .field("reason", &self.reason)
            .field("properties", &self.properties)
            .field("source", &self.source.as_ref().map(|s| s.to_string()))
            .field("context", &self.context)
            .finish()
    }
}
impl Error {
    /// Creates a new [`Box<Error>`](Error), which will be unexpected if the provided info has a server error status
    pub fn new(info: impl ErrorInfo + Send + Sync + 'static) -> Box<Self> {
        let info = Arc::new(info);
        Box::new(Self {
            unexpected: info.status().is_server_error(),
            info,
            reason: None,
            properties: None,
            source: None,
            context: SpanTrace::capture(),
        })
    }

    /// Creates a new internal server error
    pub fn internal(reason: impl Into<String>) -> Box<Self> {
        Self::new(GenericErrorCode::InternalServerError).with_reason(reason)
    }

    /// Marks this error as unexpected
    pub fn unexpected(mut self: Box<Self>) -> Box<Self> {
        self.unexpected = true;
        self
    }

    /// Marks this error as expected
    pub fn expected(mut self: Box<Self>) -> Box<Self> {
        self.unexpected = false;
        self
    }

    /// Updates the unexpected flag of the error
    pub fn with_unexpected(mut self: Box<Self>, unexpected: bool) -> Box<Self> {
        self.unexpected = unexpected;
        self
    }

    /// Updates the reason of the error
    pub fn with_reason(mut self: Box<Self>, reason: impl Into<String>) -> Box<Self> {
        self.reason = Some(reason.into());
        self
    }

    /// Updates the source of the error
    pub fn with_source<S: fmt::Display + Send + Sync + 'static>(mut self: Box<Self>, source: S) -> Box<Self> {
        self.source = Some(Arc::new(source));
        self
    }

    /// Appends an string property to the error
    pub fn with_str_property(mut self: Box<Self>, key: &str, value: impl Into<String>) -> Box<Self> {
        self.properties
            .get_or_insert_with(HashMap::new)
            .insert(key.to_string(), serde_json::Value::String(value.into()));
        self
    }

    /// Appends a property to the error
    pub fn with_property(mut self: Box<Self>, key: &str, value: serde_json::Value) -> Box<Self> {
        self.properties
            .get_or_insert_with(HashMap::new)
            .insert(key.to_string(), value);
        self
    }

    /// Returns the error info
    pub fn info(&self) -> &dyn ErrorInfo {
        self.info.as_ref()
    }

    /// Returns wether this error is unexpected or not
    pub fn is_unexpected(&self) -> bool {
        self.unexpected
    }

    /// Returns the reason (if any)
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }

    /// Returns the internal properties
    pub fn properties(&self) -> Option<&HashMap<String, serde_json::Value>> {
        self.properties.as_ref()
    }

    /// Returns the reason if any or the default error code message otherwise
    pub(super) fn reason_or_message(&self) -> String {
        self.reason.clone().unwrap_or(self.info.message())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = self.info.status();
        write!(
            f,
            "[{} {}] {}: {}",
            status.as_str(),
            status.canonical_reason().unwrap_or("Unknown"),
            self.info.code(),
            self.reason_or_message()
        )?;
        if f.alternate() {
            if let Some(source) = &self.source {
                write!(f, "\nCaused by: {source}")?;
            }
            write!(f, "\n{}", self.context)
        } else {
            Ok(())
        }
    }
}

/// Creates a new [`Box<Error>`](Error), which will be unexpected if the provided info has a server error status.
///
/// # Examples
///
/// ```
/// # use graphql_starter::{err, error::GenericErrorCode};
/// # let ctx = "";
/// # let id = "";
/// // We can provide a reason
/// err!("This is the reason for an unexpected internal server error");
/// err!("This is also, with formatted text: {}", id);
/// // Or some ErrorInfo
/// err!(GenericErrorCode::BadRequest);
/// err!(GenericErrorCode::Forbidden, "Not allowed");
/// err!(GenericErrorCode::NotFound, "Missing id {}", id);
/// ````
#[macro_export]
macro_rules! err (
    ($reason:literal) => {
        $crate::error::Error::internal($reason)
    };
    ($reason:literal,) => {
        $crate::error::Error::internal($reason)
    };
    ($reason:literal, $($arg:tt)+) => {
        $crate::error::Error::internal(format!($reason, $($arg)+))
    };
    ($info:expr) => {
        $crate::error::Error::new($info)
    };
    ($info:expr, $reason:literal) => {
        $crate::error::Error::new($info).with_reason($reason)
    };
    ($info:expr, $reason:literal,) => {
        $crate::error::Error::new($info).with_reason($reason)
    };
    ($info:expr, $reason:literal, $($arg:tt)+) => {
        $crate::error::Error::new($info).with_reason(format!($reason, $($arg)+))
    };
);
pub(crate) use err;

/// Utility trait to map any [`Result<T,E>`](std::result::Result) to a [`Result<T, Box<Error>>`]
pub trait MapToErr<T> {
    /// Maps the error to an internal server error
    fn map_to_internal_err(self, reason: &'static str) -> Result<T>;
    /// Maps the error to the given one
    fn map_to_err(self, code: impl ErrorInfo + Send + Sync + 'static) -> Result<T>;
    /// Maps the error to the given one with a reason
    fn map_to_err_with(self, code: impl ErrorInfo + Send + Sync + 'static, reason: &'static str) -> Result<T>;
}
impl<T, E: fmt::Display + Send + Sync + 'static> MapToErr<T> for Result<T, E> {
    fn map_to_internal_err(self, reason: &'static str) -> Result<T> {
        self.map_err(|source| Error::internal(reason).with_source(source))
    }

    fn map_to_err(self, code: impl ErrorInfo + Send + Sync + 'static) -> Result<T> {
        self.map_err(|source| Error::new(code).with_source(source))
    }

    fn map_to_err_with(self, code: impl ErrorInfo + Send + Sync + 'static, reason: &'static str) -> Result<T> {
        self.map_err(|source| Error::new(code).with_reason(reason).with_source(source))
    }
}

/// Utility trait to map any [`Option<T>`] to a [`Result<T, Box<Error>>`]
pub trait OkOrErr<T> {
    /// Transforms the option into a [Result], mapping [None] to an internal server error
    fn ok_or_internal_err(self, reason: &'static str) -> Result<T>;
    /// Transforms the option into a [Result], mapping [None] to the given error
    fn ok_or_err(self, code: impl ErrorInfo + Send + Sync + 'static) -> Result<T>;
    /// Transforms the option into a [Result], mapping [None] to the given error with a reason
    fn ok_or_err_with(self, code: impl ErrorInfo + Send + Sync + 'static, reason: &'static str) -> Result<T>;
}
impl<T> OkOrErr<T> for Option<T> {
    fn ok_or_internal_err(self, reason: &'static str) -> Result<T> {
        self.ok_or_else(|| Error::internal(reason))
    }

    fn ok_or_err(self, code: impl ErrorInfo + Send + Sync + 'static) -> Result<T> {
        self.ok_or_else(|| Error::new(code))
    }

    fn ok_or_err_with(self, code: impl ErrorInfo + Send + Sync + 'static, reason: &'static str) -> Result<T> {
        self.ok_or_else(|| Error::new(code).with_reason(reason))
    }
}

/// Utility trait to extend a [Result]
pub trait ResultExt {
    /// Marks the error side of the result as unexpected
    fn unexpected(self) -> Self;
    /// Marks the error side of the result as expected
    fn expected(self) -> Self;
    /// Appends an string property to the error side of the result
    fn with_str_property(self, key: &'static str, value: &'static str) -> Self;
}
impl<T> ResultExt for Result<T> {
    fn unexpected(self) -> Self {
        self.map_err(|err| err.unexpected())
    }

    fn expected(self) -> Self {
        self.map_err(|err| err.expected())
    }

    fn with_str_property(self, key: &'static str, value: &'static str) -> Self {
        self.map_err(|err| err.with_str_property(key, value))
    }
}
