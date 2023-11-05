use std::sync::Arc;

use async_trait::async_trait;

use super::Subject;
use crate::error::Result;

/// Authentication service
#[async_trait]
pub trait AuthenticationService<S: Subject>: Send + Sync {
    /// Header name containing the authentication header
    fn header_name(&self) -> &str;

    /// Cookie name containing the authentication cookie
    fn cookie_name(&self) -> &str;

    /// Validates if the given token or cookie is valid and returns the authenticated subject
    async fn authenticate(&self, token: Option<&str>, cookie: Option<&str>) -> Result<S>;
}

/// Authorization service
#[async_trait]
pub trait AuthorizationService<S: Subject>: Send + Sync {
    /// Validates if the _subject_ is allowed to perform the _relation_ on the _object_
    async fn authorize(&self, subject: &S, relation: &str, object: &str) -> Result<()>;
}

/// Sub-state to retrieve auth-related services.
///
/// The application state must implement [FromRef](axum::extract::FromRef) for [AuthState]
pub struct AuthState<S: Subject> {
    pub authn: Arc<dyn AuthenticationService<S>>,
    pub authz: Arc<dyn AuthorizationService<S>>,
}
