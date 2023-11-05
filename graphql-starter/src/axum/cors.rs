use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tower_http::cors::CorsLayer;

/// CORS service
#[async_trait]
pub trait CorsService: Send + Sync {
    /// Retrieves the allowed origins for this API
    fn allowed_origins(&self) -> &[String];
    /// Builds the [CorsLayer]
    fn build_cors_layer(&self) -> Result<CorsLayer>;
}

/// Sub-state to retrieve cors-related service.
///
/// The application state must implement [FromRef](axum::extract::FromRef) for [CorsState]
pub struct CorsState {
    pub cors: Arc<dyn CorsService>,
}
