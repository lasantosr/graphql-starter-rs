use std::path::Path;

use anyhow::{anyhow, Context, Result};
use axum::{extract::FromRef, routing::IntoMakeService, Router, Server};
use http::Request;
use hyper::{server::conn::AddrIncoming, Body};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

use super::CorsState;
use crate::request_id::{RequestId, RequestIdLayer};

/// Add tracing and cors layers to the given router
pub fn build_router<S>(router: Router<S>, state: S) -> Result<Router>
where
    S: Clone + Send + Sync + 'static,
    CorsState: FromRef<S>,
{
    // Extract the cors service
    let CorsState { cors } = FromRef::from_ref(&state);

    // Build common layers
    let layers = ServiceBuilder::new()
        // Generate random ids to each request
        .layer(RequestIdLayer)
        // Create a tracing span for each request with useful info
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<Body>| {
                let uri = request.uri().path();
                match request
                    .extensions()
                    .get::<RequestId>()
                    .map(ToString::to_string) {
                        Some(request_id) => tracing::info_span!(
                            "req",
                            id = %request_id,
                            method = %request.method(),
                            uri = %uri,
                        ),
                        None => tracing::info_span!(
                            "req",
                            method = %request.method(),
                            uri = %uri,
                        )
                    }
            }),
        )
        .layer(cors.build_cors_layer().context("couldn't build CORS layer")?);

    Ok(router.layer(layers).with_state(state))
}

/// Builds a new axum HTTP Server for a given [Router]
///
/// The server must be awaited in order to keep listening for incoming traffic:
///
/// ``` rust ignore
/// let server = build_http_server(router, 80).await?;
/// server.await?;
/// ```
pub async fn build_http_server(router: Router, port: u16) -> Result<Server<AddrIncoming, IntoMakeService<Router>>> {
    // Return server
    Ok(axum::Server::bind(&([0, 0, 0, 0], port).into()).serve(router.into_make_service()))
}

#[cfg(feature = "https")]
/// Builds a new axum HTTPS Server for a given [Router]
///
/// The server must be awaited in order to keep listening for incoming traffic:
///
/// ``` rust ignore
/// let server = build_https_server(router, 443, "./ssl/cert.pem", "./ssl/key.pem").await?;
/// server.await?;
/// ```
pub async fn build_https_server(
    router: Router,
    port: u16,
    cert: impl AsRef<Path>,
    key: impl AsRef<Path>,
) -> Result<impl std::future::Future<Output = Result<()>>> {
    use axum_server::tls_rustls::RustlsConfig;
    use futures_util::TryFutureExt;

    // SSL Config
    let config = RustlsConfig::from_pem_file(cert, key)
        .await
        .map_err(|err| anyhow!("Error reading SSL config: {err}"))?;

    // Return
    Ok(axum_server::bind_rustls(([0, 0, 0, 0], port).into(), config)
        .serve(router.into_make_service())
        .map_err(|err| anyhow!("Error serving http server: {err}")))
}
