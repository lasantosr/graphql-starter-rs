use std::{
    mem,
    sync::{Arc, Mutex},
};

use tracing::Subscriber;
use tracing_error::ErrorLayer;
use tracing_subscriber::{prelude::*, registry::LookupSpan, reload, EnvFilter, Layer, Registry};

use super::MakeWriterInterceptor;
use crate::error::{MapToErr, Result};

trait FnUpdateTracing: FnMut(&str) -> Result<()> + Send + Sync {}
impl<T> FnUpdateTracing for T where T: FnMut(&str) -> Result<()> + Send + Sync {}

#[derive(Clone)]
/// The tracing context.
///
/// This context can be cloned cheaply, as it contains an [Arc] inside, and will point to the same context.
pub struct TracingContext {
    inner: Arc<Mutex<InnerCtx>>,
}

/// Inner tracing context for [TracingContext]
struct InnerCtx {
    default_env_filter: String,
    active_env_filter: String,
    fn_update_filter: Box<dyn FnUpdateTracing>,
}

impl TracingContext {
    fn new(ctx: InnerCtx) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ctx)),
        }
    }

    /// Retrieves the default [EnvFilter]
    pub fn default_filter(&self) -> String {
        let ctx = self.inner.lock().expect("poisoned lock");
        ctx.default_env_filter.clone()
    }

    /// Retrieves the currently active [EnvFilter]
    pub fn active_filter(&self) -> String {
        let ctx = self.inner.lock().expect("poisoned lock");
        ctx.active_env_filter.clone()
    }

    /// Updates the active [EnvFilter], returning the previously active one
    pub fn update_active_filter(&mut self, new_env_filter: impl Into<String>) -> Result<String> {
        let new_env_filter = new_env_filter.into();
        let mut ctx = self.inner.lock().expect("poisoned lock");

        ctx.fn_update_filter.as_mut()(&new_env_filter)?;
        Ok(mem::replace(&mut ctx.active_env_filter, new_env_filter))
    }

    /// Updates the default [EnvFilter], returning the previously default filter
    pub fn update_default_filter(&mut self, new_default_env_filter: impl Into<String>) -> String {
        let new_default_env_filter = new_default_env_filter.into();
        let mut ctx = self.inner.lock().expect("poisoned lock");

        mem::replace(&mut ctx.default_env_filter, new_default_env_filter)
    }
}

/// Initializes the global tracing subscriber
///
/// **A global tracing subscriber can be set only once and will panic if already set**
pub fn initialize_tracing<L>(layer: L)
where
    L: Layer<Registry> + Send + Sync,
{
    // Initialize subscriber
    tracing_subscriber::registry().with(layer).init();
}

/// Generates the default compact tracing layer with the given filter
pub fn tracing_layer<T>(env_filter: impl AsRef<str>) -> impl Layer<T>
where
    T: Subscriber,
    for<'span> T: LookupSpan<'span>,
{
    // Build the layer
    let layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_filter(EnvFilter::new(env_filter.as_ref()));

    // Return the layer
    ErrorLayer::default().and_then(layer)
}

/// Generates a default tracing layer that can be updated from its [TracingContext]
pub fn dynamic_tracing_layer<T>(env_filter: impl Into<String>) -> (impl Layer<T>, TracingContext)
where
    T: Subscriber,
    for<'span> T: LookupSpan<'span>,
{
    let env_filter = env_filter.into();

    // Build the layer and handler
    let (layer, handler) = reload::Layer::new(
        tracing_subscriber::fmt::layer()
            .compact()
            .with_filter(EnvFilter::new(env_filter.clone())),
    );

    // Build the context
    let context = TracingContext::new(InnerCtx {
        default_env_filter: env_filter.clone(),
        active_env_filter: env_filter,
        fn_update_filter: Box::new(move |new_env_filter| {
            handler
                .modify(|layer| *layer.filter_mut() = EnvFilter::new(new_env_filter))
                .map_to_internal_err("Couldn't update tracing filter")
        }),
    });

    // Return the layer and its context
    (ErrorLayer::default().and_then(layer), context)
}

/// Generates a default tracing layer with the given filter that can be intercepted with its [MakeWriterInterceptor]
pub fn intercepted_tracing_layer<T>(
    env_filter: impl AsRef<str>,
    accumulate: usize,
    stream_buffer: usize,
) -> (impl Layer<T>, MakeWriterInterceptor)
where
    T: Subscriber,
    for<'span> T: LookupSpan<'span>,
{
    // Build the interceptor writer
    let make_writer = MakeWriterInterceptor::new(accumulate, stream_buffer);

    // Build the layer
    let layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_writer(make_writer.clone())
        .with_filter(EnvFilter::new(env_filter.as_ref()));

    // Return the layer
    (ErrorLayer::default().and_then(layer), make_writer)
}

/// Generates a default tracing layer that can be updated from its [TracingContext] and intercepted with its
/// [MakeWriterInterceptor]
pub fn dynamic_intercepted_tracing_layer<T>(
    env_filter: impl Into<String>,
    accumulate: usize,
    stream_buffer: usize,
) -> (impl Layer<T>, TracingContext, MakeWriterInterceptor)
where
    T: Subscriber,
    for<'span> T: LookupSpan<'span>,
{
    let env_filter = env_filter.into();

    // Build the interceptor writer
    let make_writer = MakeWriterInterceptor::new(accumulate, stream_buffer);

    // Build the intercepted layer and handler
    let (layer, handler) = reload::Layer::new(
        tracing_subscriber::fmt::layer()
            .compact()
            .with_writer(make_writer.clone())
            .with_filter(EnvFilter::new(env_filter.clone())),
    );

    // Build the context
    let context = TracingContext::new(InnerCtx {
        default_env_filter: env_filter.clone(),
        active_env_filter: env_filter,
        fn_update_filter: Box::new(move |new_env_filter| {
            handler
                .modify(|layer| *layer.filter_mut() = EnvFilter::new(new_env_filter))
                .map_to_internal_err("Couldn't update tracing filter")
        }),
    });

    // Return the layer and its context
    (ErrorLayer::default().and_then(layer), context, make_writer)
}
