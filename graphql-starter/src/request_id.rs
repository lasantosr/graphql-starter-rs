//! Based on https://github.com/imbolc/tower-request-id, but allowing to generate an [Uuid] from te [RequestId]

use std::{
    fmt,
    task::{Context, Poll},
};

use http::Request;
use tower::{Layer, Service};
use ulid::Ulid;
use uuid::Uuid;

/// A new type around [`ulid::Ulid`]
#[derive(Clone, Copy, Debug)]
pub struct RequestId(Ulid);

impl RequestId {
    fn new() -> Self {
        Self(Ulid::new())
    }
}

impl From<RequestId> for Ulid {
    fn from(value: RequestId) -> Self {
        value.0
    }
}

impl From<RequestId> for Uuid {
    fn from(value: RequestId) -> Self {
        Uuid::from_u128(value.0.0)
    }
}

impl AsRef<Ulid> for &RequestId {
    fn as_ref(&self) -> &Ulid {
        &self.0
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        let mut buffer = [0; ulid::ULID_LEN];
        write!(f, "{}", self.0.array_to_str(&mut buffer))
    }
}

/// Middleware to use [`RequestId`]
#[derive(Clone, Debug)]
pub struct RequestIdService<S> {
    inner: S,
}

impl<S> RequestIdService<S> {
    fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<B, S> Service<Request<B>> for RequestIdService<S>
where
    S: Service<Request<B>>,
{
    type Error = S::Error;
    type Future = S::Future;
    type Response = S::Response;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        let id = RequestId::new();
        req.extensions_mut().insert(id);
        self.inner.call(req)
    }
}

/// Layer to apply [`RequestIdService`] middleware.
#[derive(Clone, Debug)]
pub struct RequestIdLayer;

impl<S> Layer<S> for RequestIdLayer {
    type Service = RequestIdService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestIdService::new(inner)
    }
}
