use std::collections::HashMap;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use super::Error;
use crate::axum::extract::Json;

pub type ApiResult<T, E = Box<ApiError>> = std::result::Result<T, E>;

/// An RFC-7807 compatible error implementing axum's [IntoResponse]
#[derive(Debug, Serialize)]
pub struct ApiError {
    /// A short, human-readable title for the general error type
    title: String,
    /// Conveying the HTTP status code
    #[serde(serialize_with = "serialize_status_u16")]
    status: StatusCode,
    /// A human-readable description of the specific error
    detail: String,
    /// Additional information about the error
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    info: HashMap<String, String>,
    /// Additional details for each one of the errors encountered
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    errors: HashMap<String, serde_json::Value>,
}

impl ApiError {
    /// Builds a new error from the detail message
    pub fn new(status: StatusCode, detail: impl Into<String>) -> Self {
        ApiError {
            title: status.canonical_reason().unwrap_or("Internal server error").to_owned(),
            status,
            detail: detail.into(),
            info: Default::default(),
            errors: Default::default(),
        }
    }

    /// Modify the title
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Extend the error with additional information
    pub fn with_info(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.info.insert(key.into(), value.into());
        self
    }

    /// Extend the error with additional information about errors
    pub fn with_error_info(mut self, field: impl Into<String>, info: serde_json::Value) -> Self {
        self.errors.insert(field.into(), info);
        self
    }

    /// Boxes this error
    pub fn boxed(self) -> Box<Self> {
        Box::new(self)
    }

    /// Retrieves the error title
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Retrieves the status code
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// Retrieves the error detail
    pub fn detail(&self) -> &str {
        &self.detail
    }

    /// Retrieves the error info
    pub fn info(&self) -> &HashMap<String, String> {
        &self.info
    }

    /// Retrieves the internal errors
    pub fn errors(&self) -> &HashMap<String, serde_json::Value> {
        &self.errors
    }
}

impl From<Box<Error>> for ApiError {
    fn from(err: Box<Error>) -> Self {
        (*err).into()
    }
}
impl From<Box<Error>> for Box<ApiError> {
    fn from(err: Box<Error>) -> Self {
        (*err).into()
    }
}

impl<T> From<T> for Box<ApiError>
where
    T: Into<Error>,
{
    fn from(error: T) -> Self {
        ApiError::from(error).boxed()
    }
}

impl<T> From<T> for ApiError
where
    T: Into<Error>,
{
    fn from(error: T) -> Self {
        let error: Error = error.into();

        // Trace error before losing context information, this should usually happen just before returning to clients
        if error.unexpected {
            tracing::error!("{error:#}");
        } else if tracing::event_enabled!(tracing::Level::DEBUG) {
            tracing::warn!("{error:#}")
        } else {
            tracing::warn!("{error}")
        }

        // Build the ApiError
        let mut ret = ApiError::new(error.info.status(), error.info.message());

        // Extend the error info to allow for i18n
        ret = ret.with_info("errorCode", error.info.code());
        ret = ret.with_info("rawMessage", error.info.raw_message());
        for (key, value) in error.info.fields() {
            ret = ret.with_info(key, value);
        }

        // Extend with the error properties
        if let Some(properties) = error.properties {
            for (key, value) in properties {
                ret = ret.with_error_info(key, value);
            }
        }

        ret
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self)).into_response()
    }
}
impl IntoResponse for Box<ApiError> {
    fn into_response(self) -> Response {
        (self.status, Json(self)).into_response()
    }
}

fn serialize_status_u16<S>(status: &StatusCode, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_u16(status.as_u16())
}
