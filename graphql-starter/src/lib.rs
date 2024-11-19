mod maybe_option;
pub use maybe_option::*;

pub mod axum;
pub mod error;
pub mod pagination;
pub mod queried_fields;
pub mod request_id;
pub mod serde;
pub mod timeout;

pub use error::{ApiError, Error, Result};
#[cfg(feature = "graphql")]
pub use error::{GraphQLError, GraphQLResult};

#[cfg(feature = "ansi")]
pub mod ansi;

#[cfg(feature = "auth")]
pub mod auth;

#[cfg(feature = "config")]
pub mod config;

#[cfg(feature = "tracing")]
pub mod tracing;

#[cfg(feature = "graphql")]
pub mod graphql;

#[cfg(feature = "sqlx")]
pub mod sqlx;

#[cfg(feature = "macros")]
pub use graphql_starter_macros::*;

pub mod macros;

/// Re-exported crates
pub mod crates {
    #[cfg(feature = "paste")]
    pub mod paste {
        pub use ::paste::*;
    }
    #[cfg(feature = "https")]
    pub mod axum_server {
        pub use ::axum_server::*;
    }
    #[cfg(feature = "https")]
    pub mod rcgen {
        pub use ::rcgen::*;
    }
}
