use std::{marker::PhantomData, sync::Arc};

use async_graphql::{Context, Guard, Result};

use crate::{
    auth::{AuthErrorCode, AuthorizationService, Subject},
    error::GraphQLError,
};

/// Authorization [Guard].
///
/// This guard will use the `Arc<Option<Subject>>` and `Arc<dyn AuthorizationService<Subject>>` from the GraphQL context
/// to authorize an action, failing if they're not available.
pub struct AuthGuard<S: Subject> {
    relation: &'static str,
    object: &'static str,
    // Needed for the compiler
    sub_type: PhantomData<S>,
}

impl<S: Subject> AuthGuard<S> {
    /// Creates a new authorization guard for a given relation of an object.
    pub fn new(relation: &'static str, object: &'static str) -> Self {
        AuthGuard {
            relation,
            object,
            sub_type: PhantomData,
        }
    }
}

impl<S: Subject> Guard for AuthGuard<S> {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        let sub = ctx.data::<Arc<Option<S>>>().map_err(GraphQLError::from)?.as_ref();
        match sub {
            Some(sub) => {
                let authz = ctx
                    .data::<Arc<dyn AuthorizationService<S>>>()
                    .map_err(GraphQLError::from)?;
                Ok(authz.authorize(sub, self.relation, self.object).await?)
            }
            None => Err(GraphQLError::from((AuthErrorCode::AuthMissing, "The subject must be authenticated")).into()),
        }
    }
}
