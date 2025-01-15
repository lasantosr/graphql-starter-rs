use std::collections::HashMap;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use error_info::ErrorInfo;
use http::{header::IntoHeaderName, HeaderMap, HeaderValue};
use serde::Serialize;

use super::{Error, GenericErrorCode};
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
    /// Additional headers to be sent with the response
    #[serde(skip)]
    headers: Option<HeaderMap>,
}

impl ApiError {
    /// Builds a new error from the detail message
    pub fn new(status: StatusCode, detail: impl Into<String>) -> Box<Self> {
        Box::new(ApiError {
            title: status
                .canonical_reason()
                .unwrap_or(GenericErrorCode::InternalServerError.raw_message())
                .to_owned(),
            status,
            detail: detail.into(),
            info: Default::default(),
            errors: Default::default(),
            headers: None,
        })
    }

    /// Builds a new [ApiError] from the core [Error]
    #[allow(clippy::boxed_local)]
    pub fn from_err(err: Box<Error>) -> Box<Self> {
        let err = *err;

        // Trace error before losing context information, this should usually happen just before returning to clients
        if err.unexpected {
            tracing::error!("{err:#}");
        } else if tracing::event_enabled!(tracing::Level::DEBUG) {
            tracing::warn!("{err:#}")
        } else {
            tracing::warn!("{err}")
        }

        // Build the ApiError
        let mut ret = ApiError::new(err.info.status(), err.info.message());

        // Extend the error info to allow for i18n
        ret = ret.with_info("errorCode", err.info.code());
        ret = ret.with_info("rawMessage", err.info.raw_message());
        for (key, value) in err.info.fields() {
            if key == "errorCode" || key == "rawMessage" {
                tracing::error!("Error '{}' contains a reserved property: {}", err.info.code(), key);
                continue;
            }
            ret = ret.with_info(key, value);
        }

        // Extend with the error properties
        if let Some(properties) = err.properties {
            for (key, value) in properties {
                ret = ret.with_error_info(key, value);
            }
        }

        ret
    }

    /// Modify the title
    pub fn with_title(mut self: Box<Self>, title: impl Into<String>) -> Box<Self> {
        self.title = title.into();
        self
    }

    /// Extend the error with additional information
    pub fn with_info(mut self: Box<Self>, key: impl Into<String>, value: impl Into<String>) -> Box<Self> {
        self.info.insert(key.into(), value.into());
        self
    }

    /// Extend the error with additional information about errors
    pub fn with_error_info(mut self: Box<Self>, field: impl Into<String>, info: serde_json::Value) -> Box<Self> {
        self.errors.insert(field.into(), info);
        self
    }

    /// Extend the error with an additional header
    pub fn with_header(mut self: Box<Self>, key: impl IntoHeaderName, value: impl TryInto<HeaderValue>) -> Box<Self> {
        if let Ok(value) = value.try_into() {
            let headers = self.headers.get_or_insert_with(Default::default);
            headers.append(key, value);
        }
        self
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

    /// Retrieves the additional headers
    pub fn headers(&self) -> &Option<HeaderMap> {
        &self.headers
    }
}

impl From<Box<Error>> for Box<ApiError> {
    fn from(err: Box<Error>) -> Self {
        ApiError::from_err(err)
    }
}

impl IntoResponse for Box<ApiError> {
    fn into_response(mut self) -> Response {
        if let Some(headers) = self.headers.take() {
            (self.status, headers, Json(self)).into_response()
        } else {
            (self.status, Json(self)).into_response()
        }
    }
}

fn serialize_status_u16<S>(status: &StatusCode, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_u16(status.as_u16())
}
