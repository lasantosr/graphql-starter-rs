use std::marker::PhantomData;

use async_trait::async_trait;
use axum::extract::{FromRef, FromRequestParts, State};
use http::request::Parts;

use super::{AuthErrorCode, AuthState, Subject};
use crate::error::{ApiError, MapToErr, Result};

/// This extractor will try to authenticate the request by inspecting both the authentication header and cookie.
///
/// It will add a new extension with the optional subject ([`Option<Subject>`](Subject)).
pub struct CheckAuth<S: Subject> {
    // This field is needed in order to preserve type generics to the compiler
    sub_type: PhantomData<S>,
}

#[async_trait]
impl<S, St> FromRequestParts<St> for CheckAuth<S>
where
    S: Subject,
    St: Send + Sync,
    AuthState<S>: FromRef<St>,
{
    type Rejection = Box<ApiError>;

    async fn from_request_parts(parts: &mut Parts, state: &St) -> Result<Self, Self::Rejection> {
        // Extract the auth sub-state
        let State(AuthState { authn, authz: _ }) = State::from_request_parts(parts, state)
            .await
            .map_to_internal_err("infallible")?;

        // Extract the auth header (if any)
        let auth_header_name = authn.header_name();
        let auth_token = parts
            .headers
            .get(auth_header_name)
            .map(|v| {
                v.to_str().map_to_err(
                    AuthErrorCode::AuthMalformedAuthHeader {
                        auth_header: auth_header_name.into(),
                    },
                    "Couldn't parse auth header value",
                )
            })
            .transpose()?;

        // Extract the auth cookie (if any)
        let auth_cookie_name = authn.cookie_name();
        let auth_cookie_value = parts
            .headers
            .get(http::header::COOKIE)
            .map(|v| {
                v.to_str()
                    .map_to_err(AuthErrorCode::AuthMalformedCookies, "Couldn't parse request cookies")
            })
            .transpose()?
            .and_then(|cookies| {
                cookies
                    .split("; ")
                    .find_map(|cookie| cookie.strip_prefix(&format!("{auth_cookie_name}=")))
            });

        // Authenticate the subject
        let sub = if auth_token.is_none() && auth_cookie_value.is_none() {
            None
        } else {
            let subject = authn.authenticate(auth_token, auth_cookie_value).await?;
            tracing::trace!("Authenticated as {subject}");
            Some(subject)
        };
        parts.extensions.insert(sub);

        Ok(Self { sub_type: PhantomData })
    }
}
