//! Generic error types ready for web servers

crate::using! {
    pub core,
    pub api
}

#[cfg(feature = "graphql")]
crate::using!(pub graphql);
