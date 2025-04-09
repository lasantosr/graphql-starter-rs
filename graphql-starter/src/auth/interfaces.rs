use std::fmt;

use auto_impl::auto_impl;

use crate::error::Result;

/// Trait to identify authenticated subjects
#[auto_impl(Box, Arc)]
pub trait Subject: fmt::Display + Send + Sync + Sized + Clone + 'static {}

/// Authentication service
#[auto_impl(Box, Arc)]
#[trait_variant::make(Send)]
pub trait AuthenticationService<S: Subject>: Send + Sync + Sized + Clone + 'static {
    /// Header name containing the authentication header
    fn header_name(&self) -> &str;

    /// Cookie name containing the authentication cookie
    fn cookie_name(&self) -> &str;

    /// Validates if the given token or cookie is valid and returns the authenticated subject
    async fn authenticate(&self, token: Option<&str>, cookie: Option<&str>) -> Result<S>;
}

/// Authorization service
#[auto_impl(Box, Arc)]
#[trait_variant::make(Send)]
pub trait AuthorizationService<S: Subject>: Send + Sync + Sized + Clone + 'static {
    /// Validates if the _subject_ is allowed to perform the _relation_ on the _object_
    async fn authorize(&self, subject: &S, relation: &str, object: &str) -> Result<()>;
}

/// Trait implemented by the application State to provide specific auth service types.
pub trait AuthState<S: Subject>: Send + Sync + 'static {
    /// The concrete Authentication Service type
    type Authn: AuthenticationService<S>;

    /// The concrete Authorization Service type
    type Authz: AuthorizationService<S>;

    /// Retrieves the authentication service
    fn authn(&self) -> &Self::Authn;

    /// Retrieves the authorization service
    fn authz(&self) -> &Self::Authz;
}
