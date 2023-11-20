use std::{any::Any, sync::Arc};

use async_graphql::{ErrorExtensions, Name};
use error_info::ErrorInfo;
use indexmap::IndexMap;
use tracing_error::SpanTrace;

use super::{Error, GenericErrorCode};

/// GraphQL Result that represents either success ([`Ok`]) or failure ([`Err`])
pub type GraphQLResult<T, E = Box<GraphQLError>> = std::result::Result<T, E>;

/// GraphQL error
#[derive(Clone)]
pub enum GraphQLError {
    Async(async_graphql::Error, SpanTrace),
    Custom(Error),
}

impl GraphQLError {
    /// Creates a new [GraphQLError]
    pub fn new(info: impl ErrorInfo + Send + Sync + 'static, unexpected: bool) -> Self {
        Self::Custom(Error::new(info, unexpected))
    }

    /// Creates a new internal server error
    pub fn internal(reason: impl Into<String>) -> Self {
        Self::Custom(Error::internal(reason))
    }

    /// Appends a property to the error
    pub fn with_property(self, key: &str, value: serde_json::Value) -> Self {
        match self {
            Self::Async(err, ctx) => {
                let err = err.extend_with(|_, e| match async_graphql::Value::try_from(value) {
                    Ok(value) => e.set(key, value),
                    Err(err) => tracing::error!("Couldn't deserialize error value: {err}"),
                });
                Self::Async(err, ctx)
            }
            Self::Custom(err) => {
                let err = err.with_property(key, value);
                Self::Custom(err)
            }
        }
    }

    /// Boxes this error
    pub fn boxed(self) -> Box<Self> {
        Box::new(self)
    }

    /// Checks wether this error is unexpected or not
    fn is_unexpected(&self) -> bool {
        match self {
            // errors from the graphql lib are unexpected
            GraphQLError::Async(_, _) => true,
            GraphQLError::Custom(err) => err.unexpected,
        }
    }

    /// Returns the string representation of the error
    pub fn to_string(&self, include_context: bool) -> String {
        match self {
            GraphQLError::Async(err, context) => {
                let code = GenericErrorCode::InternalServerError;
                let status = code.status();
                if include_context {
                    format!(
                        "[{} {}] {}: {}\n{}",
                        status.as_str(),
                        status.canonical_reason().unwrap_or("Unknown"),
                        code.code(),
                        &err.message,
                        &context
                    )
                } else {
                    format!(
                        "[{} {}] {}: {}",
                        status.as_str(),
                        status.canonical_reason().unwrap_or("Unknown"),
                        code.code(),
                        &err.message
                    )
                }
            }
            GraphQLError::Custom(err) => {
                if include_context {
                    format!("{err:#}")
                } else {
                    format!("{err}")
                }
            }
        }
    }
}

impl From<async_graphql::Error> for GraphQLError {
    fn from(err: async_graphql::Error) -> Self {
        GraphQLError::Async(err, SpanTrace::capture())
    }
}
impl From<async_graphql::Error> for Box<GraphQLError> {
    fn from(err: async_graphql::Error) -> Self {
        GraphQLError::from(err).boxed()
    }
}

impl From<Box<Error>> for GraphQLError {
    fn from(err: Box<Error>) -> Self {
        (*err).into()
    }
}
impl From<Box<Error>> for Box<GraphQLError> {
    fn from(err: Box<Error>) -> Self {
        (*err).into()
    }
}

impl<T> From<T> for GraphQLError
where
    T: Into<Error>,
{
    fn from(err: T) -> Self {
        GraphQLError::Custom(err.into())
    }
}
impl<T> From<T> for Box<GraphQLError>
where
    T: Into<Error>,
{
    fn from(err: T) -> Self {
        GraphQLError::from(err).boxed()
    }
}

impl From<Box<GraphQLError>> for async_graphql::Error {
    fn from(value: Box<GraphQLError>) -> Self {
        (*value).into()
    }
}
impl From<GraphQLError> for async_graphql::Error {
    fn from(e: GraphQLError) -> Self {
        // Trace the error when converting to async_graphql error, which is done just before responding to requests
        if e.is_unexpected() {
            tracing::error!("{}", e.to_string(true))
        } else if tracing::event_enabled!(tracing::Level::DEBUG) {
            tracing::warn!("{}", e.to_string(true))
        } else {
            tracing::warn!("{}", e.to_string(false))
        }

        // Convert type
        let (gql_err, err_info): (async_graphql::Error, Arc<dyn ErrorInfo + Send + Sync + 'static>) = match e {
            GraphQLError::Async(mut err, _) => {
                // Hide the message and provide generic internal error info
                err.source = Some(Arc::new(err.message));
                err.message = "Internal server error".into();
                (err, Arc::new(GenericErrorCode::InternalServerError))
            }
            GraphQLError::Custom(err) => {
                let source = err.source.map(|s| {
                    let source: Arc<dyn Any + Send + Sync> = Arc::new(s);
                    source
                });
                (
                    async_graphql::Error {
                        message: err.info.message(),
                        source,
                        extensions: None,
                    }
                    .extend_with(|_, e| {
                        if let Some(prop) = err.properties {
                            for (k, v) in prop.into_iter() {
                                if k == "statusCode"
                                    || k == "statusKind"
                                    || k == "errorCode"
                                    || k == "rawMessage"
                                    || k == "messageFields"
                                {
                                    tracing::error!("Error '{}' contains a reserved property: {}", err.info.code(), k);
                                    continue;
                                }
                                match async_graphql::Value::try_from(v) {
                                    Ok(v) => e.set(k, v),
                                    Err(err) => tracing::error!("Couldn't deserialize error value: {err}"),
                                }
                            }
                        }
                    }),
                    err.info,
                )
            }
        };
        // Append error info properties
        gql_err.extend_with(|_, e| {
            let status = err_info.status();
            e.set("statusCode", status.as_u16());
            if let Some(reason) = status.canonical_reason() {
                e.set("statusKind", reason);
            }
            e.set("errorCode", err_info.code());
            e.set("rawMessage", err_info.raw_message());
            let fields = err_info.fields();
            if !fields.is_empty() {
                let fields_map = IndexMap::from_iter(fields.into_iter().map(|(k, v)| (Name::new(k), v.into())));
                e.set("messageFields", fields_map);
            }
        })
    }
}
