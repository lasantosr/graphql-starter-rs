use std::marker::PhantomData;

use async_graphql::{Context, Guard, Result};

use crate::{
    auth::{AuthErrorCode, AuthState, AuthorizationService, Subject},
    error::{err, GraphQLError},
};

/// Authorization [Guard].
///
/// This guard will use the `Option<S>` and the state from the GraphQL context
/// to authorize an action, failing if they're not available.
pub struct AuthGuard<S: Subject, St: AuthState<S>> {
    relation: &'static str,
    object: &'static str,
    // Needed for the compiler
    sub_type: PhantomData<S>,
    state_type: PhantomData<St>,
}

impl<S: Subject, St: AuthState<S>> AuthGuard<S, St> {
    /// Creates a new authorization guard for a given relation of an object.
    pub fn new(relation: &'static str, object: &'static str) -> Self {
        AuthGuard {
            relation,
            object,
            sub_type: PhantomData,
            state_type: PhantomData,
        }
    }
}

impl<S: Subject, St: AuthState<S>> Guard for AuthGuard<S, St> {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        let sub = ctx.data::<Option<S>>().map_err(Box::<GraphQLError>::from)?.as_ref();
        match sub {
            Some(sub) => {
                let state = ctx.data::<St>().map_err(Box::<GraphQLError>::from)?;
                Ok(state.authz().authorize(sub, self.relation, self.object).await?)
            }
            None => Err(
                GraphQLError::from_err(err!(AuthErrorCode::AuthMissing, "The subject must be authenticated")).into(),
            ),
        }
    }
}
