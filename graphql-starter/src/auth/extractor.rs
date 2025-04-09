use axum::extract::{FromRequestParts, OptionalFromRequestParts};
use http::request::Parts;

use super::{AuthErrorCode, AuthState, AuthenticationService, Subject};
use crate::error::{err, ApiError, MapToErr, OkOrErr, Result};

/// This extractor will authenticate the request by inspecting both the authentication header and cookie.
///
/// It also implements [OptionalFromRequestParts] so it can be optionally extracted, returning [None] if there is no
/// auth header or cookie, but failing if they're present but not valid.
pub struct Auth<S: Subject>(pub S);

impl<S, St> OptionalFromRequestParts<St> for Auth<S>
where
    S: Subject,
    St: AuthState<S> + Send + Sync,
{
    type Rejection = Box<ApiError>;

    async fn from_request_parts(parts: &mut Parts, state: &St) -> Result<Option<Self>, Self::Rejection> {
        // Extract the auth header (if any)
        let auth_header_name = state.authn().header_name();
        let auth_token = parts
            .headers
            .get(auth_header_name)
            .map(|v| {
                v.to_str().map_err(|err| {
                    err!(
                        AuthErrorCode::AuthMalformedAuthHeader {
                            auth_header: auth_header_name.into(),
                        },
                        "Couldn't parse auth header value"
                    )
                    .with_source(err)
                })
            })
            .transpose()?
            .filter(|t| !t.is_empty());

        // Extract the auth cookie (if any)
        let auth_cookie_name = state.authn().cookie_name();
        let auth_cookie_value = parts
            .headers
            .get(http::header::COOKIE)
            .map(|v| {
                v.to_str()
                    .map_to_err_with(AuthErrorCode::AuthMalformedCookies, "Couldn't parse request cookies")
            })
            .transpose()?
            .and_then(|cookies| {
                cookies
                    .split("; ")
                    .find_map(|cookie| cookie.strip_prefix(&format!("{auth_cookie_name}=")))
            })
            .filter(|c| !c.is_empty());

        // Authenticate the subject
        if auth_token.is_none() && auth_cookie_value.is_none() {
            Ok(None)
        } else {
            let subject = match state.authn().authenticate(auth_token, auth_cookie_value).await {
                Ok(s) => s,
                Err(err) => {
                    let is_invalid_token = err.info().code() == "AUTH_INVALID_TOKEN";
                    let mut err: Box<ApiError> = err.into();
                    // If the authentication fails because the token is invalid, remove the auth cookie if set
                    // If the cookie is HttpOnly, clients are not able to remove it manually when invalid
                    if auth_cookie_value.is_some() && is_invalid_token {
                        err = err.with_header(
                            "Set-Cookie",
                            format!("{auth_cookie_name}=invalid; Expires=Thu, 01 Jan 1970 00:00:00 GMT"),
                        );
                    }
                    return Err(err);
                }
            };
            tracing::trace!("Authenticated as {subject}");
            Ok(Some(Self(subject)))
        }
    }
}

impl<S, St> FromRequestParts<St> for Auth<S>
where
    S: Subject,
    St: AuthState<S> + Send + Sync,
{
    type Rejection = Box<ApiError>;

    async fn from_request_parts(parts: &mut Parts, state: &St) -> Result<Self, Self::Rejection> {
        Ok(<Self as OptionalFromRequestParts<St>>::from_request_parts(parts, state)
            .await?
            .ok_or_err_with(AuthErrorCode::AuthMissing, "The subject must be authenticated")?)
    }
}
