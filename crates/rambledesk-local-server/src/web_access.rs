use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::{
        Request, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderValue, Response, StatusCode, Uri, header},
    middleware::{self, Next},
    response::IntoResponse,
    routing::{get, post},
};
use futures::{SinkExt, StreamExt};
use rambledesk_core::{ApplicationChangeHub, ApplicationCommandFacade, ApplicationEvent};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, broadcast};
use tokio_util::sync::CancellationToken;

use crate::{
    WebSessionAuthorization, application_router,
    web_security::{bearer_credential, has_exact_host_and_origin},
};

pub const EVENT_PROTOCOL: &str = "rambledesk-events";
pub const EVENT_CREDENTIAL_PROTOCOL_PREFIX: &str = "rambledesk-session.";

pub trait WebSessionAuthenticator: Send + Sync {
    fn authorize(&self, session_token: &str) -> Option<WebSessionAuthorization>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebAccessRouteConfig {
    allowed_host: String,
    allowed_origin: String,
    max_event_connections: usize,
    max_http_requests: usize,
}

impl WebAccessRouteConfig {
    pub fn new(
        allowed_host: impl Into<String>,
        allowed_origin: impl Into<String>,
        max_event_connections: usize,
        max_http_requests: usize,
    ) -> Result<Self, String> {
        let allowed_host = allowed_host.into();
        let allowed_origin = allowed_origin.into();
        let origin = allowed_origin
            .parse::<Uri>()
            .map_err(|_| "Web Access origin must be an absolute HTTP(S) origin".to_owned())?;
        if !matches!(origin.scheme_str(), Some("http" | "https"))
            || origin.authority().map(|value| value.as_str()) != Some(allowed_host.as_str())
            || origin.path() != "/"
            || origin.query().is_some()
            || max_event_connections == 0
            || max_http_requests == 0
        {
            return Err(
                "Web Access Host and Origin must be same-origin and connection limit non-zero"
                    .to_owned(),
            );
        }
        Ok(Self {
            allowed_host,
            allowed_origin: allowed_origin.trim_end_matches('/').to_owned(),
            max_event_connections,
            max_http_requests,
        })
    }
}

#[derive(Clone)]
struct WebAccessState {
    authenticator: Arc<dyn WebSessionAuthenticator>,
    changes: Arc<ApplicationChangeHub>,
    config: WebAccessRouteConfig,
    event_connections: Arc<Semaphore>,
    http_requests: Arc<Semaphore>,
    lifecycle: CancellationToken,
}

pub fn web_access_router(
    commands: Arc<ApplicationCommandFacade>,
    changes: Arc<ApplicationChangeHub>,
    authenticator: Arc<dyn WebSessionAuthenticator>,
    config: WebAccessRouteConfig,
    lifecycle: CancellationToken,
) -> Router {
    let state = WebAccessState {
        authenticator,
        changes: changes.clone(),
        event_connections: Arc::new(Semaphore::new(config.max_event_connections)),
        http_requests: Arc::new(Semaphore::new(config.max_http_requests)),
        lifecycle,
        config,
    };
    let runtime_routes = Router::new()
        .route("/health", post(health))
        .route("/events", get(events))
        .with_state(state.clone());

    Router::new()
        .merge(runtime_routes)
        .merge(application_router(commands, changes))
        .layer(middleware::from_fn_with_state(state, authorize_web_access))
}

async fn health(State(state): State<WebAccessState>) -> impl IntoResponse {
    Json(state.changes.metadata())
}

async fn events(
    State(state): State<WebAccessState>,
    axum::extract::Extension(authorization): axum::extract::Extension<WebSessionAuthorization>,
    websocket: WebSocketUpgrade,
) -> Response<Body> {
    let Ok(permit) = state.event_connections.clone().try_acquire_owned() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    // Subscribe before reading the ready revision so no invalidation can fall
    // between the initial snapshot ledger and the live stream.
    let (ready, receiver) = state.changes.subscribe_with_ready();
    websocket
        .protocols([EVENT_PROTOCOL])
        .on_upgrade(move |socket| {
            event_session(
                socket,
                ready,
                receiver,
                authorization,
                state.lifecycle,
                permit,
            )
        })
}

async fn event_session(
    mut socket: WebSocket,
    ready: ApplicationEvent,
    mut receiver: broadcast::Receiver<rambledesk_core::ApplicationInvalidation>,
    mut authorization: WebSessionAuthorization,
    lifecycle: CancellationToken,
    _permit: OwnedSemaphorePermit,
) {
    if send_event(&mut socket, &ready).await.is_err() {
        return;
    }
    loop {
        tokio::select! {
            () = lifecycle.cancelled() => {
                let _ = socket.close().await;
                return;
            },
            () = authorization.revoked() => {
                let _ = socket.close().await;
                return;
            },
            message = socket.next() => match message {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return,
                _ => {}
            },
            invalidation = receiver.recv() => match invalidation {
                Ok(invalidation) => {
                    if send_event(&mut socket, &ApplicationEvent::from(invalidation)).await.is_err() {
                        return;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_))
                | Err(broadcast::error::RecvError::Closed) => {
                    let _ = socket.close().await;
                    return;
                }
            }
        }
    }
}

async fn send_event(socket: &mut WebSocket, event: &ApplicationEvent) -> Result<(), axum::Error> {
    let payload = serde_json::to_string(event).expect("application event must serialize");
    socket.send(Message::Text(payload.into())).await
}

async fn authorize_web_access(
    State(state): State<WebAccessState>,
    request: Request,
    next: Next,
) -> Response<Body> {
    if !has_exact_host_and_origin(
        request.headers(),
        &state.config.allowed_host,
        &state.config.allowed_origin,
    ) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let websocket = request.uri().path().ends_with("/events");
    let _http_permit = if websocket {
        None
    } else {
        match state.http_requests.clone().try_acquire_owned() {
            Ok(permit) => Some(permit),
            Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        }
    };
    let credential = if websocket {
        websocket_credential(request.headers().get(header::SEC_WEBSOCKET_PROTOCOL))
    } else {
        bearer_credential(request.headers().get(header::AUTHORIZATION))
    };
    let Some(authorization) = credential.and_then(|token| state.authenticator.authorize(token))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let mut request = request;
    request.extensions_mut().insert(authorization);
    // Authorization is an admission lease. Once a command handler starts, a
    // concurrent stop/rotation/expiry must not rewrite an already-committed
    // mutation into 401; subsequent requests and the event socket are revoked.
    next.run(request).await
}

fn websocket_credential(value: Option<&HeaderValue>) -> Option<&str> {
    let protocols = value?.to_str().ok()?.split(',').map(str::trim);
    let mut saw_event_protocol = false;
    let mut credential = None;
    for protocol in protocols {
        if protocol == EVENT_PROTOCOL {
            saw_event_protocol = true;
        } else if let Some(token) = protocol.strip_prefix(EVENT_CREDENTIAL_PROTOCOL_PREFIX)
            && (token.is_empty()
                || !token
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                || credential.replace(token).is_some())
        {
            return None;
        }
    }
    saw_event_protocol.then_some(credential).flatten()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::{Extension, routing::post};
    use tokio::{net::TcpListener, sync::Notify};

    use crate::{DurableWebAccessToken, WebSessionManager};

    use super::*;

    struct SlowMutation {
        started: Notify,
        release: Notify,
        committed: AtomicUsize,
    }

    async fn slow_mutation(Extension(control): Extension<Arc<SlowMutation>>) -> StatusCode {
        control.committed.fetch_add(1, Ordering::SeqCst);
        control.started.notify_one();
        control.release.notified().await;
        StatusCode::NO_CONTENT
    }

    #[test]
    fn route_config_requires_same_origin_and_a_connection_budget() {
        assert!(
            WebAccessRouteConfig::new("127.0.0.1:4000", "http://127.0.0.1:4000", 4, 16).is_ok()
        );
        assert!(
            WebAccessRouteConfig::new("127.0.0.1:4000", "http://localhost:4000", 4, 16).is_err()
        );
        assert!(
            WebAccessRouteConfig::new("127.0.0.1:4000", "http://127.0.0.1:4000", 0, 16).is_err()
        );
        assert!(
            WebAccessRouteConfig::new("127.0.0.1:4000", "http://127.0.0.1:4000", 4, 0).is_err()
        );
    }

    #[test]
    fn websocket_token_requires_stable_and_single_credential_protocols() {
        let valid = HeaderValue::from_static("rambledesk-events, rambledesk-session.abc_123");
        assert_eq!(websocket_credential(Some(&valid)), Some("abc_123"));
        let missing_stable = HeaderValue::from_static("rambledesk-session.abc_123");
        assert_eq!(websocket_credential(Some(&missing_stable)), None);
        let duplicate = HeaderValue::from_static(
            "rambledesk-events, rambledesk-session.a, rambledesk-session.b",
        );
        assert_eq!(websocket_credential(Some(&duplicate)), None);
    }

    #[tokio::test]
    async fn admitted_mutation_keeps_its_result_when_the_session_is_revoked() {
        let sessions = Arc::new(WebSessionManager::new(
            DurableWebAccessToken::parse("a".repeat(64)).expect("durable token"),
            "runtime-a",
        ));
        let token = sessions
            .issue_session(&"a".repeat(64))
            .expect("session token");
        let config = WebAccessRouteConfig::new("127.0.0.1:37643", "http://127.0.0.1:37643", 2, 1)
            .expect("route config");
        let state = WebAccessState {
            authenticator: sessions.clone(),
            changes: Arc::new(ApplicationChangeHub::with_runtime_generation("runtime-a")),
            event_connections: Arc::new(Semaphore::new(2)),
            http_requests: Arc::new(Semaphore::new(1)),
            lifecycle: CancellationToken::new(),
            config,
        };
        let control = Arc::new(SlowMutation {
            started: Notify::new(),
            release: Notify::new(),
            committed: AtomicUsize::new(0),
        });
        let router = Router::new()
            .route("/slow", post(slow_mutation))
            .layer(Extension(control.clone()))
            .layer(middleware::from_fn_with_state(state, authorize_web_access));
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("address");
        let server = tokio::spawn(axum::serve(listener, router).into_future());
        let second_token = token.clone();
        let request = tokio::spawn(async move {
            reqwest::Client::new()
                .post(format!("http://{address}/slow"))
                .header(header::HOST, "127.0.0.1:37643")
                .header(header::ORIGIN, "http://127.0.0.1:37643")
                .bearer_auth(token)
                .send()
                .await
                .expect("request")
        });

        control.started.notified().await;
        let saturated = reqwest::Client::new()
            .post(format!("http://{address}/slow"))
            .header(header::HOST, "127.0.0.1:37643")
            .header(header::ORIGIN, "http://127.0.0.1:37643")
            .bearer_auth(second_token)
            .send()
            .await
            .expect("saturated request");
        assert_eq!(saturated.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
        sessions.revoke_all();
        control.release.notify_one();
        let response = request.await.expect("request task");
        assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);
        assert_eq!(control.committed.load(Ordering::SeqCst), 1);
        server.abort();
    }
}
