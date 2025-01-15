//! Based on https://github.com/tower-rs/tower-http/blob/main/tower-http/src/timeout/service.rs, but allowing to
//! customize the response

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use axum::response::{IntoResponse, Response};
use error_info::ErrorInfo;
use http::Request;
use pin_project_lite::pin_project;
use tokio::time::Sleep;
use tower::{Layer, Service};

use crate::error::{ApiError, Error};

/// Layer that applies the [`Timeout`] middleware which apply a timeout to requests.
///
/// See the [module docs](super) for an example.
#[derive(Debug, Clone, Copy)]
pub struct TimeoutLayer<T: ErrorInfo + Send + Sync + Copy + 'static> {
    timeout: Duration,
    response: T,
}

impl<T> TimeoutLayer<T>
where
    T: ErrorInfo + Send + Sync + Copy + 'static,
{
    /// Creates a new [`TimeoutLayer`].
    pub fn new(timeout: Duration, response: T) -> Self {
        TimeoutLayer { timeout, response }
    }
}

impl<T, S> Layer<S> for TimeoutLayer<T>
where
    T: ErrorInfo + Send + Sync + Copy + 'static,
{
    type Service = Timeout<S, T>;

    fn layer(&self, inner: S) -> Self::Service {
        Timeout::new(inner, self.timeout, self.response)
    }
}

/// Middleware which apply a timeout to requests.
///
/// If the request does not complete within the specified timeout it will be aborted and a `408
/// Request Timeout` response will be sent.
///
/// See the [module docs](super) for an example.
#[derive(Debug, Clone, Copy)]
pub struct Timeout<S, T> {
    inner: S,
    timeout: Duration,
    response: T,
}

impl<S, T> Timeout<S, T>
where
    T: ErrorInfo + Send + Sync + Copy + 'static,
{
    /// Creates a new [`Timeout`].
    pub fn new(inner: S, timeout: Duration, response: T) -> Self {
        Self {
            inner,
            timeout,
            response,
        }
    }
}

impl<S, T, ReqBody> Service<Request<ReqBody>> for Timeout<S, T>
where
    S: Service<Request<ReqBody>, Response = Response>,
    T: ErrorInfo + Send + Sync + Copy + 'static,
{
    type Error = S::Error;
    type Future = ResponseFuture<S::Future, T>;
    type Response = S::Response;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let sleep = tokio::time::sleep(self.timeout);
        ResponseFuture {
            inner: self.inner.call(req),
            sleep,
            response: self.response,
        }
    }
}

pin_project! {
    /// Response future for [`Timeout`].
    pub struct ResponseFuture<F,T> {
        #[pin]
        inner: F,
        #[pin]
        sleep: Sleep,
        #[pin]
        response: T,
    }
}

impl<F, T, E> Future for ResponseFuture<F, T>
where
    F: Future<Output = Result<Response, E>>,
    T: ErrorInfo + Send + Sync + Copy + 'static,
{
    type Output = Result<Response, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        if this.sleep.poll(cx).is_ready() {
            let err = ApiError::from_err(Error::new(*this.response));
            return Poll::Ready(Ok(err.into_response()));
        }

        this.inner.poll(cx)
    }
}
