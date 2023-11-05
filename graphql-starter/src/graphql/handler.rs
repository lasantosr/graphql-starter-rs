use async_graphql::http::{Credentials, GraphiQLSource};
use axum::response::{Html, IntoResponse};

/// Handler that renders a GraphQL playground on the given path to explore the GraphQL API.
pub async fn graphql_playground_handler(path: String, title: &str) -> impl IntoResponse {
    Html(
        GraphiQLSource::build()
            .endpoint(&path)
            .subscription_endpoint(&format!("{path}/ws"))
            .title(title)
            .credentials(Credentials::SameOrigin)
            .finish(),
    )
}

#[cfg(feature = "auth")]
mod auth {
    use async_graphql::{
        http::ALL_WEBSOCKET_PROTOCOLS, BatchResponse, Data, ObjectType, Response, Schema, SubscriptionType,
    };
    use async_graphql_axum::{GraphQLBatchRequest, GraphQLProtocol, GraphQLResponse, GraphQLWebSocket};
    use axum::{
        body::BoxBody,
        extract::{FromRequestParts, WebSocketUpgrade},
        response::IntoResponse,
    };

    use crate::{
        auth::{AuthErrorCode, AuthState, Subject},
        axum::{extract::Extension, CorsState},
        error::{ApiError, GenericErrorCode, MapToErr},
        request_id::RequestId,
    };

    /// Handler for [batch requests](https://www.apollographql.com/blog/apollo-client/performance/batching-client-graphql-queries/).
    ///
    /// Both [`Option<Subject>`](Subject) and [RequestId] will be added to the GraphQL context before executing the
    /// request on the schema.
    ///
    /// This handler expects three extensions:
    /// - `Schema<Query, Mutation, Subscription>` with the GraphQL [Schema]
    /// - `Option<Subject>` with the subject (see [CheckAuth](crate::auth::CheckAuth))
    /// - `RequestId` with the request id (see [RequestIdLayer](crate::request_id::RequestIdLayer))
    pub async fn graphql_batch_handler<S: Subject, Query, Mutation, Subscription>(
        Extension(schema): Extension<Schema<Query, Mutation, Subscription>>,
        Extension(subject): Extension<Option<S>>,
        Extension(request_id): Extension<RequestId>,
        req: GraphQLBatchRequest,
    ) -> GraphQLResponse
    where
        Query: ObjectType + 'static,
        Mutation: ObjectType + 'static,
        Subscription: SubscriptionType + 'static,
    {
        let req = req.into_inner();
        // Log request operations
        if tracing::event_enabled!(tracing::Level::TRACE) {
            let op_names = req
                .iter()
                .map(|r| r.operation_name.as_deref().unwrap_or("Unknown"))
                .collect::<Vec<_>>()
                .join(", ");
            tracing::trace!("request operations: {op_names}")
        }
        // Include the subject and request_id from the Axum extension into the GraphQL context as well and execute the
        // requests
        let mut res = schema.execute_batch(req.data(subject).data(request_id)).await;
        // Include the request id if any error is found
        match &mut res {
            BatchResponse::Single(res) => include_request_id(res, &request_id),
            BatchResponse::Batch(responses) => {
                for res in responses {
                    include_request_id(res, &request_id);
                }
            }
        }
        res.into()
    }

    /// Handler for GraphQL [subscriptions](https://www.apollographql.com/docs/react/data/subscriptions/).
    ///
    /// **Note**: This handler only works with `GET` method, it must always be used with [`get`](axum::routing::get).
    ///
    /// Both [`Option<Subject>`](Subject) and [RequestId] will be added to the GraphQL context before executing the
    /// request on the schema.
    ///
    /// This handler expects two extensions:
    /// - `Schema<Query, Mutation, Subscription>` with the GraphQL [Schema]
    /// - `RequestId` with the request id (see [RequestIdLayer](crate::request_id::RequestIdLayer))
    ///
    /// Authentication will be performed using the same criteria than [CheckAuth](crate::auth::CheckAuth) extractor,
    /// retrieving the Cookie from the `GET` request and the token from the
    /// [`GQL_CONNECTION_INIT` message](https://github.com/apollographql/subscriptions-transport-ws/blob/master/PROTOCOL.md#gql_connection_init).
    pub async fn graphql_subscription_handler<Query, Mutation, Subscription, S: Subject, State, B>(
        Extension(schema): Extension<Schema<Query, Mutation, Subscription>>,
        Extension(request_id): Extension<RequestId>,
        axum::extract::State(CorsState { cors }): axum::extract::State<CorsState>,
        axum::extract::State(AuthState { authn, authz: _ }): axum::extract::State<AuthState<S>>,
        req: http::Request<B>,
    ) -> axum::http::Response<BoxBody>
    where
        Query: ObjectType + 'static,
        Mutation: ObjectType + 'static,
        Subscription: SubscriptionType + 'static,
    {
        let (mut parts, _body) = req.into_parts();

        // Retrieve `Origin` header set by browsers
        let origin_header = match parts
            .headers
            .get(http::header::ORIGIN)
            .map(|v| {
                v.to_str()
                    .map_to_err(GenericErrorCode::BadRequest, "Couldn't parse request cookies")
            })
            .transpose()
        {
            Ok(o) => o,
            Err(err) => return ApiError::from(err).into_response(),
        };
        // If it's present, check it's allowed
        if let Some(origin_header) = origin_header {
            if !cors.allowed_origins().iter().any(|o| o == origin_header) {
                return ApiError::from((GenericErrorCode::Forbidden, "The origin is not allowed")).into_response();
            }
        }

        // Retrieve token & cookie names
        let auth_header_name = authn.header_name().to_lowercase();
        let auth_cookie_name = authn.cookie_name().to_owned();

        // Retrieve the auth cookie value
        let cookies = match parts
            .headers
            .get(http::header::COOKIE)
            .map(|v| {
                v.to_str()
                    .map_to_err(AuthErrorCode::AuthMalformedCookies, "Couldn't parse request cookies")
            })
            .transpose()
        {
            Ok(c) => c,
            Err(err) => return ApiError::from(err).into_response(),
        };
        let auth_cookie_value = cookies
            .and_then(|cookies| {
                cookies
                    .split("; ")
                    .find_map(|cookie| cookie.strip_prefix(&format!("{auth_cookie_name}=")))
            })
            .map(|s| s.to_owned());

        // Based on https://github.com/async-graphql/async-graphql/blob/master/integrations/axum/src/subscription.rs
        // Extract GraphQL WebSocket protocol
        let protocol = match GraphQLProtocol::from_request_parts(&mut parts, &()).await {
            Ok(protocol) => protocol,
            Err(err) => return err.into_response(),
        };
        // Prepare upgrade connection from HTTPS to WSS
        let upgrade = match WebSocketUpgrade::from_request_parts(&mut parts, &()).await {
            Ok(protocol) => protocol,
            Err(err) => return err.into_response(),
        };

        // Finalize upgrading connection
        upgrade
            .protocols(ALL_WEBSOCKET_PROTOCOLS)
            .on_upgrade(move |stream| {
                // Forward the stream to the GraphQL websocket
                GraphQLWebSocket::new(stream, schema.clone(), protocol)
                    .on_connection_init(move |payload| {
                        // Authenticate the subject on connection init
                        async move {
                            let mut data = Data::default();
                            // Retrieve auth token from the payload
                            let auth_token = payload.as_object().and_then(|payload| {
                                payload
                                    .iter()
                                    .find(|(k, _)| k.to_lowercase() == auth_header_name)
                                    .and_then(|(_, v)| v.as_str())
                            });
                            // Authenticate the subject
                            let subject = authn.authenticate(auth_token, auth_cookie_value.as_deref()).await?;
                            tracing::trace!("Authenticated as {subject}");

                            // Include the subject and request_id from the Axum extension into the GraphQL context
                            data.insert(Some(subject.clone()));
                            data.insert(request_id);

                            Ok(data)
                        }
                    })
                    .serve()
            })
            .into_response()
    }

    /// Includes the request id extension on the response errors (if any)
    fn include_request_id(res: &mut Response, id: &RequestId) {
        for e in &mut res.errors {
            e.extensions
                .get_or_insert_with(Default::default)
                .set("requestId", id.to_string())
        }
    }
}
#[cfg(feature = "auth")]
pub use auth::*;
