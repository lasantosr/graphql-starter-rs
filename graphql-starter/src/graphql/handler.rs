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
    use async_graphql::{
        http::ALL_WEBSOCKET_PROTOCOLS, BatchRequest, BatchResponse, Data, ObjectType, Response, Schema,
        SubscriptionType,
    };
    use async_graphql_axum::{GraphQLProtocol, GraphQLResponse, GraphQLWebSocket};
    use auto_impl::auto_impl;
    use axum::{
        extract::{FromRequestParts, State, WebSocketUpgrade},
        response::IntoResponse,
    };
    use futures_util::{stream::FuturesOrdered, StreamExt};
    use tracing::Instrument;

    use crate::{
        auth::{Auth, AuthErrorCode, AuthState, AuthenticationService, Subject},
        axum::{
            extract::{AcceptLanguage, Extension},
            CorsService, CorsState,
        },
        error::{err, ApiError, GenericErrorCode, MapToErr},
        graphql::GraphQLBatchRequest,
        request_id::RequestId,
    };

    /// Middleware to customize the data attached to each GraphQL request.
    #[auto_impl(Box, Arc)]
    pub trait RequestDataMiddleware<S: Subject>: Send + Sync + Sized + Clone + 'static {
        /// Customize the given request data, inserting or modifying the content.
        fn customize_request_data(&self, subject: &Option<S>, accept_language: &AcceptLanguage, data: &mut Data);
    }
    impl<S: Subject> RequestDataMiddleware<S> for () {
        fn customize_request_data(&self, _subject: &Option<S>, _accept_language: &AcceptLanguage, _data: &mut Data) {}
    }

    /// Handler for [batch requests](https://www.apollographql.com/blog/apollo-client/performance/batching-client-graphql-queries/).
    ///
    /// [RequestId], [`Option<Subject>`](Subject) and [AcceptLanguage] will be added to the GraphQL context before
    /// executing the request on the schema.
    ///
    /// This handler expects two extensions:
    /// - `Schema<Query, Mutation, Subscription>` with the GraphQL [Schema]
    /// - `RequestId` with the request id (see [RequestIdLayer](crate::request_id::RequestIdLayer))
    ///
    /// And optionally:
    /// - `RequestDataMiddleware<Subject>` with the [RequestDataMiddleware]
    pub async fn graphql_batch_handler<S: Subject, M: RequestDataMiddleware<S>, Query, Mutation, Subscription>(
        Extension(schema): Extension<Schema<Query, Mutation, Subscription>>,
        Extension(request_id): Extension<RequestId>,
        middleware: Option<Extension<M>>,
        subject: Option<Auth<S>>,
        accept_language: AcceptLanguage,
        req: GraphQLBatchRequest,
    ) -> GraphQLResponse
    where
        Query: ObjectType + 'static,
        Mutation: ObjectType + 'static,
        Subscription: SubscriptionType + 'static,
    {
        let mut req = req.into_inner();
        let subject = subject.map(|s| s.0);
        // Log request operations
        if tracing::event_enabled!(tracing::Level::TRACE) {
            let op_names = req
                .iter()
                .flat_map(|r| r.operation_name.as_deref())
                .collect::<Vec<_>>()
                .join(", ");
            tracing::trace!("request operations: {op_names}")
        }
        // Call the request data middleware to include additional data
        if let Some(Extension(middleware)) = middleware {
            match &mut req {
                BatchRequest::Single(r) => {
                    middleware.customize_request_data(&subject, &accept_language, &mut r.data);
                }
                BatchRequest::Batch(b) => {
                    for r in b {
                        middleware.customize_request_data(&subject, &accept_language, &mut r.data);
                    }
                }
            }
        }
        // Include the request_id, subject and accept language into the GraphQL context
        req = req.data(request_id).data(subject).data(accept_language);
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
    /// **Note**: For HTTP/1.1 requests, this handler requires the request method to be `GET`; in later versions,
    /// `CONNECT` is used instead. To support both, it should be used with [`any`](axum::routing::any).
    ///
    /// [RequestId], [`Option<Subject>`](Subject) and [AcceptLanguage] will be added to the GraphQL context before
    /// executing the request on the schema.
    ///
    /// This handler expects two extensions:
    /// - `Schema<Query, Mutation, Subscription>` with the GraphQL [Schema]
    /// - `RequestId` with the request id (see [RequestIdLayer](crate::request_id::RequestIdLayer))
    ///
    /// And optionally:
    /// - `RequestDataMiddleware<Subject>` with the [RequestDataMiddleware]
    ///
    /// Authentication will be performed using the same criteria than [Auth](crate::auth::Auth) extractor,
    /// retrieving the Cookie from the `GET` request and the token from the
    /// [`GQL_CONNECTION_INIT` message](https://github.com/apollographql/subscriptions-transport-ws/blob/master/PROTOCOL.md#gql_connection_init).
    pub async fn graphql_subscription_handler<
        Query,
        Mutation,
        Subscription,
        S: Subject,
        M: RequestDataMiddleware<S>,
        St: AuthState<S> + CorsState,
        B,
    >(
        State(state): State<St>,
        Extension(schema): Extension<Schema<Query, Mutation, Subscription>>,
        Extension(request_id): Extension<RequestId>,
        middleware: Option<Extension<M>>,
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
                    .map_to_err_with(GenericErrorCode::BadRequest, "Couldn't parse request origin header")
            })
            .transpose()
        {
            Ok(o) => o,
            Err(err) => return ApiError::from_err(err).into_response(),
        };
        // If it's present, check it's allowed
        if let Some(origin_header) = origin_header {
            if !state.cors().allowed_origins().iter().any(|o| o == origin_header) {
                return ApiError::from_err(err!(GenericErrorCode::Forbidden, "The origin is not allowed"))
                    .into_response();
            }
        }

        // Retrieve token & cookie names
        let authn = state.authn().clone();
        let auth_header_name = authn.header_name().to_lowercase();
        let auth_cookie_name = authn.cookie_name().to_owned();

        // Retrieve the auth cookie value
        let cookies = match parts
            .headers
            .get(http::header::COOKIE)
            .map(|v| {
                v.to_str()
                    .map_to_err_with(AuthErrorCode::AuthMalformedCookies, "Couldn't parse request cookies")
            })
            .transpose()
        {
            Ok(c) => c,
            Err(err) => return ApiError::from_err(err).into_response(),
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
                            let subject = Some(subject);

                            // Call the request data middleware to include additional data
                            if let Some(Extension(middleware)) = middleware {
                                middleware.customize_request_data(&subject, &accept_language, &mut data);
                            }

                            // Include the request_id, subject and accept language into the GraphQL context
                            data.insert(request_id);
                            data.insert(subject);
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
