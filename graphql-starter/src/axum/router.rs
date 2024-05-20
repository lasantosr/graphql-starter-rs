use core::future::Future;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::{body::Body, extract::FromRef, serve::WithGracefulShutdown, Router};
use http::Request;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};

use super::CorsState;
use crate::request_id::{RequestId, RequestIdLayer};

/// Add tracing and cors layers to the given router
pub fn build_router<S>(router: Router<S>, state: S, request_timeout: Duration) -> Result<Router>
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
        // Add a timeout so requests don't hang forever
        .layer(TimeoutLayer::new(request_timeout))
        // Add CORS layer as well
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
pub async fn build_http_server(
    router: Router,
    port: u16,
) -> anyhow::Result<WithGracefulShutdown<Router, Router, impl Future<Output = ()>>> {
    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .context("Can't bind TCP listener")?;
    Ok(axum::serve(listener, router).with_graceful_shutdown(shutdown_signal()))
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
    cert: impl AsRef<std::path::Path>,
    key: impl AsRef<std::path::Path>,
) -> Result<impl std::future::Future<Output = Result<()>>> {
    use axum_server::tls_rustls::RustlsConfig;

    // SSL Config
    let config = RustlsConfig::from_pem_file(cert, key)
        .await
        .map_err(|err| anyhow::anyhow!("Error reading SSL config: {err}"))?;

    // Build server
    build_https_server_with(router, port, config).await
}

#[cfg(feature = "https")]
/// Builds a new axum HTTPS Server for a given [Router] with a self-signed certificate
///
/// The server must be awaited in order to keep listening for incoming traffic:
///
/// ``` rust ignore
/// let server = build_self_signed_https_server(router, 443, ["localhost"]).await?;
/// server.await?;
/// ```
pub async fn build_self_signed_https_server(
    router: Router,
    port: u16,
    subject_alt_names: impl IntoIterator<Item = impl Into<String>>,
) -> Result<impl std::future::Future<Output = Result<()>>> {
    use axum_server::tls_rustls::RustlsConfig;
    use rcgen::CertifiedKey;

    // Generate a self-signed certificate
    let CertifiedKey { cert, key_pair } =
        rcgen::generate_simple_self_signed(subject_alt_names.into_iter().map(|n| n.into()).collect::<Vec<String>>())
            .map_err(|err| anyhow::anyhow!("Couldn't generate self-signed certificate: {err}"))?;

    // SSL Config
    let config = RustlsConfig::from_pem(cert.pem().into(), key_pair.serialize_pem().into())
        .await
        .map_err(|err| anyhow::anyhow!("Error reading SSL config: {err}"))?;

    // Build server
    build_https_server_with(router, port, config).await
}

#[cfg(feature = "https")]
/// Builds a new axum HTTPS Server for a given [Router] with the given config
///
/// The server must be awaited in order to keep listening for incoming traffic:
///
/// ``` rust ignore
/// let server = build_https_server_with(router, 443, config).await?;
/// server.await?;
/// ```
pub async fn build_https_server_with(
    router: Router,
    port: u16,
    config: axum_server::tls_rustls::RustlsConfig,
) -> Result<impl std::future::Future<Output = Result<()>>> {
    use axum_server::Handle;
    use futures_util::TryFutureExt;

    // Graceful shutdown handle
    let handle = Handle::new();
    let cloned_handle = handle.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        tracing::trace!("received graceful shutdown signal. Telling tasks to shutdown");
        cloned_handle.graceful_shutdown(Some(Duration::from_secs(10)));
    });

    // Return
    Ok(axum_server::bind_rustls(([0, 0, 0, 0], port).into(), config)
        .handle(handle)
        .serve(router.into_make_service())
        .map_err(|err| anyhow::anyhow!("Error serving http server: {err}")))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
