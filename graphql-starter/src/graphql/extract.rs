//! Manually implementing GraphQLBatchRequest to customize multipart options, as [recommended](https://github.com/async-graphql/async-graphql/issues/1220).

use std::{io::ErrorKind, marker::PhantomData};

use async_graphql::{futures_util::TryStreamExt, http::MultipartOptions, ParseRequestError};
use axum::{
    extract::{FromRef, FromRequest, Request},
    http::{self, Method},
    response::IntoResponse,
};
use tokio_util::compat::TokioAsyncReadCompatExt;

/// Extractor for GraphQL request.
pub struct GraphQLRequest<R = rejection::GraphQLRejection>(pub async_graphql::Request, PhantomData<R>);

impl<R> GraphQLRequest<R> {
    /// Unwraps the value to `async_graphql::Request`.
    #[must_use]
    pub fn into_inner(self) -> async_graphql::Request {
        self.0
    }
}

/// Rejection response types.
pub mod rejection {
    use async_graphql::ParseRequestError;
    use axum::{
        http::StatusCode,
        response::{IntoResponse, Response},
    };

    use crate::error::ApiError;

    /// Rejection used for [`GraphQLRequest`](super::GraphQLRequest).
    pub struct GraphQLRejection(pub ParseRequestError);

    impl IntoResponse for GraphQLRejection {
        fn into_response(self) -> Response {
            match self.0 {
                ParseRequestError::PayloadTooLarge => {
                    tracing::warn!("[413 Payload Too Large] Received a GraphQL request with a payload too large");
                    ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, "Payload too large").into_response()
                }
                bad_request => {
                    let msg = bad_request.to_string();
                    tracing::warn!("[400 Bad Request] {msg}");
                    ApiError::new(StatusCode::BAD_REQUEST, msg).into_response()
                }
            }
        }
    }

    impl From<ParseRequestError> for GraphQLRejection {
        fn from(err: ParseRequestError) -> Self {
            GraphQLRejection(err)
        }
    }
}

impl<S, R> FromRequest<S> for GraphQLRequest<R>
where
    S: Send + Sync,
    MultipartOptions: FromRef<S>,
    R: IntoResponse + From<ParseRequestError>,
{
    type Rejection = R;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        Ok(GraphQLRequest(
            GraphQLBatchRequest::<R>::from_request(req, state)
                .await?
                .0
                .into_single()?,
            PhantomData,
        ))
    }
}

/// Extractor for GraphQL batch request.
pub struct GraphQLBatchRequest<R = rejection::GraphQLRejection>(pub async_graphql::BatchRequest, PhantomData<R>);

impl<R> GraphQLBatchRequest<R> {
    /// Unwraps the value to `async_graphql::BatchRequest`.
    #[must_use]
    pub fn into_inner(self) -> async_graphql::BatchRequest {
        self.0
    }
}

impl<S, R> FromRequest<S> for GraphQLBatchRequest<R>
where
    S: Send + Sync,
    R: IntoResponse + From<ParseRequestError>,
    MultipartOptions: FromRef<S>,
{
    type Rejection = R;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        if req.method() == Method::GET {
            let uri = req.uri();
            let res = async_graphql::http::parse_query_string(uri.query().unwrap_or_default()).map_err(|err| {
                ParseRequestError::Io(std::io::Error::new(
                    ErrorKind::Other,
                    format!("failed to parse graphql request from uri query: {}", err),
                ))
            });
            Ok(Self(async_graphql::BatchRequest::Single(res?), PhantomData))
        } else {
            let content_type = req
                .headers()
                .get(http::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(ToString::to_string);
            let body_stream = req
                .into_body()
                .into_data_stream()
                .map_err(|err| std::io::Error::new(ErrorKind::Other, err.to_string()));
            let body_reader = tokio_util::io::StreamReader::new(body_stream).compat();
            Ok(Self(
                async_graphql::http::receive_batch_body(content_type, body_reader, FromRef::from_ref(state)).await?,
                PhantomData,
            ))
        }
    }
}
