//! Custom de/serialization helpers to use in combination with [serde's with-annotation][https://serde.rs/field-attrs.html#with].
//!
//! It's pretty much an extension of [serde_with](https://docs.rs/serde_with/latest/serde_with)

pub mod std;

#[cfg(feature = "chrono")]
pub mod chrono;
