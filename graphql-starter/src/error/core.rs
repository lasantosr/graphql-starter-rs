use std::{collections::HashMap, fmt, sync::Arc};

use error_info::ErrorInfo;
use http::StatusCode;
use tracing_error::SpanTrace;

pub type Result<T, E = Box<Error>> = std::result::Result<T, E>;

/// Generic error codes, they're usually not meant for the end-user
#[derive(ErrorInfo)]
pub enum GenericErrorCode {
    #[error(status = StatusCode::BAD_REQUEST, message = "The request is not well formed")]
    BadRequest,
    #[error(status = StatusCode::FORBIDDEN, message = "Forbidden access to the resource")]
    Forbidden,
    #[error(status = StatusCode::INTERNAL_SERVER_ERROR, message = "Internal server error")]
    InternalServerError,
}

/// This type represents an error in the service
#[derive(Clone)]
pub struct Error {
    pub(super) info: Arc<dyn ErrorInfo + Send + Sync + 'static>,
    pub(super) reason: Option<String>,
    pub(super) properties: Option<HashMap<String, String>>,
    pub(super) unexpected: bool,
    pub(super) source: Option<Arc<dyn fmt::Display + Send + Sync>>,
    pub(super) context: SpanTrace,
}
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let info_debug = f
            .debug_struct("ErrorInfo")
            .field("status", &self.info.status())
            .field("code", &self.info.code().to_string())
            .field("raw_message", &self.info.raw_message())
            .finish();

        f.debug_struct("Error")
            .field("info", &info_debug)
            .field("reason", &self.reason)
            .field("properties", &self.properties)
            .field("source", &self.source.as_ref().map(|s| s.to_string()))
            .field("context", &self.context)
            .finish()
    }
}
impl Error {
    /// Creates a new [Error]
    pub fn new(info: impl ErrorInfo + Send + Sync + 'static, unexpected: bool) -> Self {
        Self {
            info: Arc::new(info),
            reason: None,
            properties: None,
            unexpected,
            source: None,
            context: SpanTrace::capture(),
        }
    }

    /// Creates a new internal server error
    pub fn internal(reason: impl Into<String>) -> Self {
        Self::new(GenericErrorCode::InternalServerError, true).with_reason(reason)
    }

    /// Updates the reason of the error
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Updates the source of the error
    pub fn with_source<S: fmt::Display + Send + Sync + 'static>(mut self, source: S) -> Self {
        self.source = Some(Arc::new(source));
        self
    }

    /// Appends a property to the error
    pub fn with_property(mut self, key: &str, value: &str) -> Self {
        self.properties
            .get_or_insert_with(HashMap::new)
            .insert(key.to_string(), value.to_string());
        self
    }

    /// Boxes this error
    pub fn boxed(self) -> Box<Self> {
        Box::new(self)
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
    pub fn properties(&self) -> Option<&HashMap<String, String>> {
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
impl From<&str> for Error {
    fn from(reason: &str) -> Self {
        Self::internal(reason)
    }
}
impl From<String> for Error {
    fn from(reason: String) -> Self {
        Self::internal(reason)
    }
}
impl<T: ErrorInfo + Send + Sync + 'static> From<(T,)> for Error {
    fn from((code,): (T,)) -> Self {
        let status = code.status();
        Self::new(code, status.is_server_error())
    }
}
impl<T: ErrorInfo + Send + Sync + 'static, S: Into<String>> From<(T, S)> for Error {
    fn from((code, reason): (T, S)) -> Self {
        let status = code.status();
        Self::new(code, status.is_server_error()).with_reason(reason)
    }
}
impl From<&str> for Box<Error> {
    fn from(reason: &str) -> Self {
        Error::from(reason).boxed()
    }
}
impl From<String> for Box<Error> {
    fn from(reason: String) -> Self {
        Error::from(reason).boxed()
    }
}
impl<T: ErrorInfo + Send + Sync + 'static> From<(T,)> for Box<Error> {
    fn from(t: (T,)) -> Self {
        Error::from(t).boxed()
    }
}
impl<T: ErrorInfo + Send + Sync + 'static, S: Into<String>> From<(T, S)> for Box<Error> {
    fn from(t: (T, S)) -> Self {
        Error::from(t).boxed()
    }
}

/// Utility trait to map any [`Result<T,E>`](std::result::Result) to a [`Result<T, Box<Error>>`]
pub trait MapToErr<T> {
    /// Maps the error to an internal server error
    fn map_to_internal_err(self, reason: impl Into<String>) -> Result<T>;
    /// Maps the error to the given one
    fn map_to_err(self, code: impl ErrorInfo + Send + Sync + 'static, reason: impl Into<String>) -> Result<T>;
}
impl<T, E: fmt::Display + Send + Sync + 'static> MapToErr<T> for Result<T, E> {
    fn map_to_internal_err(self, reason: impl Into<String>) -> Result<T> {
        self.map_err(|err| Error::internal(reason.into()).with_source(err).boxed())
    }

    fn map_to_err(self, code: impl ErrorInfo + Send + Sync + 'static, reason: impl Into<String>) -> Result<T> {
        self.map_err(|err| {
            let unexpected = code.status().is_server_error();
            Error::new(code, unexpected)
                .with_reason(reason.into())
                .with_source(err)
                .boxed()
        })
    }
}

/// Utility trait to extend a [Result]
pub trait ResultExt {
    /// Appends a property to the error side of the result
    fn with_property(self, key: &str, value: &str) -> Self;
}
impl<T> ResultExt for Result<T> {
    fn with_property(self, key: &str, value: &str) -> Self {
        self.map_err(|err| err.with_property(key, value).boxed())
    }
}
