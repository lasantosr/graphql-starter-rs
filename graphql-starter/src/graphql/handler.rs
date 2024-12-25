use async_graphql::http::{AltairSource, Credentials, GraphiQLSource};
use axum::response::{Html, IntoResponse};

/// Handler that renders a GraphiQL playground on the given path to explore the API.
pub async fn graphiql_playground_handler(path: String, title: &str) -> impl IntoResponse {
    Html(
        GraphiQLSource::build()
            .endpoint(&path)
            .subscription_endpoint(&format!("{path}/ws"))
            .title(title)
            .credentials(Credentials::SameOrigin)
            .header("x-requested-with", "graphiql")
            .finish(),
    )
}

/// Handler that renders an Altair GraphQL playground on the given path to explore the API.
pub async fn altair_playground_handler(path: String, title: &str) -> impl IntoResponse {
    Html(
        AltairSource::build()
            .title(title)
            .options(serde_json::json!({
                "endpointURL": path,
                "subscriptionsEndpoint": format!("{path}/ws"),
                "subscriptionsProtocol": "wss",
                "disableAccount": true,
                "initialHeaders": {
                    "x-requested-with": "altair"
                },
                "initialSettings": {
                    "addQueryDepthLimit": 1,
                    "request.withCredentials": true,
                    "plugin.list": ["altair-graphql-plugin-graphql-explorer"],
                    "schema.reloadOnStart": true,
                }
            }))
            .finish(),
    )
}

#[cfg(feature = "auth")]
mod auth {
    use std::sync::Arc;

    use async_graphql::{
        http::ALL_WEBSOCKET_PROTOCOLS, BatchRequest, BatchResponse, Data, ObjectType, Response, Schema,
        SubscriptionType,
    };
    use async_graphql_axum::{GraphQLProtocol, GraphQLResponse, GraphQLWebSocket};
    use axum::{
        extract::{FromRequestParts, WebSocketUpgrade},
        response::IntoResponse,
    };
    use futures_util::{stream::FuturesOrdered, StreamExt};
    use tracing::Instrument;

    use crate::{
        auth::{AuthErrorCode, AuthState, Subject},
        axum::{
            extract::{AcceptLanguage, Extension},
            CorsState,
        },
        error::{ApiError, GenericErrorCode, MapToErr},
        graphql::GraphQLBatchRequest,
        request_id::RequestId,
    };

    /// Middleware to customize the data attached to each GraphQL request.
    pub trait RequestDataMiddleware<S: Subject>: Send + Sync + 'static {
        /// Customize the given request data, inserting or modifying the content.
        fn customize_request_data(&self, subject: &Arc<Option<S>>, accept_language: &AcceptLanguage, data: &mut Data);
    }

    /// Handler for [batch requests](https://www.apollographql.com/blog/apollo-client/performance/batching-client-graphql-queries/).
    ///
    /// Both [`Arc<Option<Subject>>`](Subject) and [RequestId] will be added to the GraphQL context before executing the
    /// request on the schema.
    ///
    /// This handler expects four extensions:
    /// - `Schema<Query, Mutation, Subscription>` with the GraphQL [Schema]
    /// - `Arc<RequestDataMiddleware>` with the [RequestDataMiddleware]
    /// - `Arc<Option<Subject>>` with the subject (see [CheckAuth](crate::auth::CheckAuth))
    /// - `RequestId` with the request id (see [RequestIdLayer](crate::request_id::RequestIdLayer))
    pub async fn graphql_batch_handler<S: Subject, M: RequestDataMiddleware<S>, Query, Mutation, Subscription>(
        Extension(schema): Extension<Schema<Query, Mutation, Subscription>>,
        Extension(data_middleware): Extension<Arc<M>>,
        Extension(subject): Extension<Arc<Option<S>>>,
        Extension(request_id): Extension<RequestId>,
        accept_language: AcceptLanguage,
        req: GraphQLBatchRequest,
    ) -> GraphQLResponse
    where
        Query: ObjectType + 'static,
        Mutation: ObjectType + 'static,
        Subscription: SubscriptionType + 'static,
    {
        let mut req = req.into_inner();
        // Log request operations
        if tracing::event_enabled!(tracing::Level::TRACE) {
            let op_names = req
                .iter()
                .map(|r| r.operation_name.as_deref().unwrap_or("Unknown"))
                .collect::<Vec<_>>()
                .join(", ");
            tracing::trace!("request operations: {op_names}")
        }
        // Call the request data middleware to include additional data
        match &mut req {
            BatchRequest::Single(r) => {
                data_middleware.customize_request_data(&subject, &accept_language, &mut r.data);
            }
            BatchRequest::Batch(b) => {
                for r in b {
                    data_middleware.customize_request_data(&subject, &accept_language, &mut r.data);
                }
            }
        }
        // Include the subject and request_id from the Axum extension into the GraphQL context as well
        req = req.data(subject.clone()).data(request_id);
        // Include also the extracted accept language header
        req = req.data(accept_language);
        // Execute the requests, instrumenting them with the operation name (if present)
        let mut res = match req {
            BatchRequest::Single(request) => {
                let span = if let Some(op) = &request.operation_name {
                    tracing::info_span!("gql", %op)
                } else {
                    tracing::info_span!("gql")
                };
                BatchResponse::Single(schema.execute(request).instrument(span).await)
            }
            BatchRequest::Batch(requests) => BatchResponse::Batch(
                FuturesOrdered::from_iter(requests.into_iter().map(|request| {
                    let span = if let Some(op) = &request.operation_name {
                        tracing::info_span!("gql", %op)
                    } else {
                        tracing::info_span!("gql")
                    };
                    schema.execute(request).instrument(span)
                }))
                .collect()
                .await,
            ),
        };
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
    /// Both [`Arc<Option<Subject>>`](Subject) and [RequestId] will be added to the GraphQL context before executing the
    /// request on the schema.
    ///
    /// This handler expects three extensions:
    /// - `Schema<Query, Mutation, Subscription>` with the GraphQL [Schema]
    /// - `Arc<RequestDataMiddleware>` with the [RequestDataMiddleware]
    /// - `RequestId` with the request id (see [RequestIdLayer](crate::request_id::RequestIdLayer))
    ///
    /// Authentication will be performed using the same criteria than [CheckAuth](crate::auth::CheckAuth) extractor,
    /// retrieving the Cookie from the `GET` request and the token from the
    /// [`GQL_CONNECTION_INIT` message](https://github.com/apollographql/subscriptions-transport-ws/blob/master/PROTOCOL.md#gql_connection_init).
    pub async fn graphql_subscription_handler<
        Query,
        Mutation,
        Subscription,
        S: Subject,
        M: RequestDataMiddleware<S>,
        B,
    >(
        Extension(schema): Extension<Schema<Query, Mutation, Subscription>>,
        Extension(data_middleware): Extension<Arc<M>>,
        Extension(request_id): Extension<RequestId>,
        axum::extract::State(CorsState { cors }): axum::extract::State<CorsState>,
        axum::extract::State(AuthState { authn, authz: _ }): axum::extract::State<AuthState<S>>,
        accept_language: AcceptLanguage,
        req: http::Request<B>,
    ) -> axum::response::Response
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
                            let subject = Arc::new(Some(subject));

                            // Call the request data middleware to include additional data
                            data_middleware.customize_request_data(&subject, &accept_language, &mut data);

                            // Include the subject and request_id from the Axum extension into the GraphQL context
                            data.insert(subject.clone());
                            data.insert(request_id);

                            // Include also the extracted accept language header
                            data.insert(accept_language);

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
