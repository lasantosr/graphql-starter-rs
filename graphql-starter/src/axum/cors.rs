use anyhow::Result;
use auto_impl::auto_impl;
use tower_http::cors::CorsLayer;

/// CORS service
#[auto_impl(Box, Arc)]
#[trait_variant::make(Send)]
pub trait CorsService: Send + Sync + Sized + 'static {
    /// Retrieves the allowed origins for this API
    fn allowed_origins(&self) -> &[String];
    /// Builds the [CorsLayer]
    fn build_cors_layer(&self) -> Result<CorsLayer>;
}

/// Trait implemented by the application State to provide cors-related services.
pub trait CorsState {
    /// The concrete CORS Service type
    type Cors: CorsService;

    /// Retrieves the CORS service
    fn cors(&self) -> &Self::Cors;
}
