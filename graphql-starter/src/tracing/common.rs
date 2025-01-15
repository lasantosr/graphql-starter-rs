use std::{
    mem,
    sync::{Arc, Mutex},
};

use anyhow::Context;
use tracing::Subscriber;
use tracing_error::ErrorLayer;
use tracing_subscriber::{filter::Targets, prelude::*, registry::LookupSpan, reload, Layer, Registry};

use super::MakeWriterInterceptor;
use crate::error::{GenericErrorCode, MapToErr, Result};

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
    default_filter: String,
    active_filter: String,
    fn_update_filter: Box<dyn FnUpdateTracing>,
}

impl TracingContext {
    fn new(ctx: InnerCtx) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ctx)),
        }
    }

    /// Retrieves the default [Targets] filter
    pub fn default_filter(&self) -> String {
        let ctx = self.inner.lock().expect("poisoned lock");
        ctx.default_filter.clone()
    }

    /// Retrieves the currently active [Targets] filter
    pub fn active_filter(&self) -> String {
        let ctx = self.inner.lock().expect("poisoned lock");
        ctx.active_filter.clone()
    }

    /// Updates the active [Targets] filter, returning the previously active one
    pub fn update_active_filter(&mut self, new_filter: impl Into<String>) -> Result<String> {
        let new_filter = new_filter.into();
        let mut ctx = self.inner.lock().expect("poisoned lock");

        ctx.fn_update_filter.as_mut()(&new_filter)?;
        Ok(mem::replace(&mut ctx.active_filter, new_filter))
    }

    /// Updates the default [Targets] filter, returning the previously default filter
    pub fn update_default_filter(&mut self, new_default_filter: impl Into<String>) -> String {
        let new_default_filter = new_default_filter.into();
        let mut ctx = self.inner.lock().expect("poisoned lock");

        mem::replace(&mut ctx.default_filter, new_default_filter)
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

/// Generates the default compact tracing layer with the given [Targets] filter
pub fn tracing_layer<T>(filter: impl AsRef<str>) -> anyhow::Result<impl Layer<T>>
where
    T: Subscriber,
    for<'span> T: LookupSpan<'span>,
{
    // Build the layer
    let layer = tracing_subscriber::fmt::layer().compact().with_filter(
        filter
            .as_ref()
            .parse::<Targets>()
            .context("Couldn't parse tracing filter")?,
    );

    // Return the layer
    Ok(ErrorLayer::default().and_then(layer))
}

/// Generates a default tracing layer with the given [Targets] filter that can be updated from its [TracingContext]
pub fn dynamic_tracing_layer<T>(filter: impl Into<String>) -> anyhow::Result<(impl Layer<T>, TracingContext)>
where
    T: Subscriber,
    for<'span> T: LookupSpan<'span>,
{
    let filter = filter.into();

    // Build the layer and handler
    let (layer, handler) = reload::Layer::new(
        tracing_subscriber::fmt::layer()
            .compact()
            .with_filter(filter.parse::<Targets>().context("Couldn't parse tracing filter")?),
    );

    // Build the context
    let context = TracingContext::new(InnerCtx {
        default_filter: filter.clone(),
        active_filter: filter,
        fn_update_filter: Box::new(move |new_filter| {
            let new_filter = new_filter
                .parse::<Targets>()
                .map_to_err_with(GenericErrorCode::BadRequest, "Couldn't parse the filter")?;
            handler
                .modify(|layer| *layer.filter_mut() = new_filter)
                .map_to_internal_err("Couldn't update tracing filter")
        }),
    });

    // Return the layer and its context
    Ok((ErrorLayer::default().and_then(layer), context))
}

/// Generates a default tracing layer with the given [Targets] filter that can be intercepted with its
/// [MakeWriterInterceptor]
pub fn intercepted_tracing_layer<T>(
    filter: impl AsRef<str>,
    accumulate: usize,
    stream_buffer: usize,
) -> anyhow::Result<(impl Layer<T>, MakeWriterInterceptor)>
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
        .with_filter(
            filter
                .as_ref()
                .parse::<Targets>()
                .context("Couldn't parse tracing filter")?,
        );

    // Return the layer
    Ok((ErrorLayer::default().and_then(layer), make_writer))
}

/// Generates a default tracing layer with the given [Targets] filter that can be updated from its [TracingContext] and
/// intercepted with its [MakeWriterInterceptor]
pub fn dynamic_intercepted_tracing_layer<T>(
    filter: impl Into<String>,
    accumulate: usize,
    stream_buffer: usize,
) -> anyhow::Result<(impl Layer<T>, TracingContext, MakeWriterInterceptor)>
where
    T: Subscriber,
    for<'span> T: LookupSpan<'span>,
{
    let filter = filter.into();

    // Build the interceptor writer
    let make_writer = MakeWriterInterceptor::new(accumulate, stream_buffer);

    // Build the intercepted layer and handler
    let (layer, handler) = reload::Layer::new(
        tracing_subscriber::fmt::layer()
            .compact()
            .with_writer(make_writer.clone())
            .with_filter(filter.parse::<Targets>().context("Couldn't parse tracing filter")?),
    );

    // Build the context
    let context = TracingContext::new(InnerCtx {
        default_filter: filter.clone(),
        active_filter: filter,
        fn_update_filter: Box::new(move |new_filter| {
            let new_filter = new_filter
                .parse::<Targets>()
                .map_to_err_with(GenericErrorCode::BadRequest, "Couldn't parse the filter")?;
            handler
                .modify(|layer| *layer.filter_mut() = new_filter)
                .map_to_internal_err("Couldn't update tracing filter")
        }),
    });

    // Return the layer and its context
    Ok((ErrorLayer::default().and_then(layer), context, make_writer))
}
