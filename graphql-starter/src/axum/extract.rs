//! Wrappers over axum's [extract](https://docs.rs/axum/latest/axum/extract/index.html), providing custom error responses.
//!
//! It avoids having to use [WithRejection](https://docs.rs/axum-extra/latest/axum_extra/extract/struct.WithRejection.html)
//! every time

use axum::{
    extract::{FromRequest, FromRequestParts, Request},
    response::{IntoResponse, Response},
};
use bytes::{BufMut, BytesMut};
use http::{header, request::Parts, HeaderValue};
use serde::{de::DeserializeOwned, Serialize};

use crate::error::{ApiError, MapToErr};

/// Wrapper over [axum::Json] to customize error responses
#[derive(Debug, Clone, Copy, Default)]
#[must_use]
pub struct Json<T>(pub T);

#[axum::async_trait]
impl<S, T> FromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Box<ApiError>;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        ::axum::Json::<T>::from_request(req, state)
            .await
            .map(|::axum::Json(value)| Json(value))
            .map_err(|err| {
                tracing::info!("Couldn't parse json request: {err}");
                ApiError::new(err.status(), err.body_text()).boxed()
            })
    }
}

impl<T> IntoResponse for Json<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        // Mimic ::axum::Json::into_response with custom error
        let mut buf = BytesMut::with_capacity(128).writer();
        match serde_json::to_writer(&mut buf, &self.0).map_to_internal_err("Error serializing response") {
            Ok(()) => (
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
                )],
                buf.into_inner().freeze(),
            )
                .into_response(),
            Err(err) => ApiError::from(err).into_response(),
        }
    }
}

/// Wrapper over [axum::extract::Query] to customize error responses
#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

#[axum::async_trait]
impl<T, S> FromRequestParts<S> for Query<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Box<ApiError>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        ::axum::extract::Query::<T>::from_request_parts(parts, state)
            .await
            .map(|::axum::extract::Query(value)| Query(value))
            .map_err(|err| {
                tracing::info!("Couldn't parse request query: {err}");
                ApiError::new(err.status(), err.body_text()).boxed()
            })
    }
}

/// Wrapper over [axum::extract::Path] to customize error responses
#[derive(Debug, Clone, Copy, Default)]
pub struct Path<T>(pub T);

#[axum::async_trait]
impl<T, S> FromRequestParts<S> for Path<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = Box<ApiError>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        ::axum::extract::Path::<T>::from_request_parts(parts, state)
            .await
            .map(|::axum::extract::Path(value)| Path(value))
            .map_err(|err| {
                tracing::warn!("Couldn't extract request path: {err}");
                ApiError::new(err.status(), err.body_text()).boxed()
            })
    }
}

/// Wrapper over [axum::Extension] to customize error responses
#[derive(Debug, Clone, Copy, Default)]
pub struct Extension<T>(pub T);

#[axum::async_trait]
impl<T, S> FromRequestParts<S> for Extension<T>
where
    T: Clone + Send + Sync + 'static,
    S: Send + Sync,
{
    type Rejection = Box<ApiError>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        ::axum::Extension::<T>::from_request_parts(parts, state)
            .await
            .map(|::axum::Extension(value)| Extension(value))
            .map_err(|err| {
                tracing::warn!("Couldn't extract extension: {err}");
                ApiError::new(err.status(), "Internal server error").boxed()
            })
    }
}

/// Extractor for an optional [Extension]
#[derive(Debug, Clone, Copy, Default)]
pub struct ExtensionOpt<T>(pub Option<T>);

#[axum::async_trait]
impl<T, S> FromRequestParts<S> for ExtensionOpt<T>
where
    T: Clone + Send + Sync + 'static,
    S: Send + Sync,
{
    type Rejection = Box<ApiError>;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(ExtensionOpt(parts.extensions.get::<T>().cloned()))
    }
}
