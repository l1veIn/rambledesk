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

use crate::application_router;

pub const EVENT_PROTOCOL: &str = "rambledesk-events";
pub const EVENT_CREDENTIAL_PROTOCOL_PREFIX: &str = "rambledesk-session.";

pub trait WebSessionAuthenticator: Send + Sync {
    fn authenticate(&self, session_token: &str) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebAccessRouteConfig {
    allowed_host: String,
    allowed_origin: String,
    max_event_connections: usize,
}

impl WebAccessRouteConfig {
    pub fn new(
        allowed_host: impl Into<String>,
        allowed_origin: impl Into<String>,
        max_event_connections: usize,
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
        })
    }
}

#[derive(Clone)]
struct WebAccessState {
    authenticator: Arc<dyn WebSessionAuthenticator>,
    changes: Arc<ApplicationChangeHub>,
    config: WebAccessRouteConfig,
    event_connections: Arc<Semaphore>,
}

pub fn web_access_router(
    commands: Arc<ApplicationCommandFacade>,
    changes: Arc<ApplicationChangeHub>,
    authenticator: Arc<dyn WebSessionAuthenticator>,
    config: WebAccessRouteConfig,
) -> Router {
    let state = WebAccessState {
        authenticator,
        changes: changes.clone(),
        event_connections: Arc::new(Semaphore::new(config.max_event_connections)),
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
        .on_upgrade(move |socket| event_session(socket, ready, receiver, permit))
}

async fn event_session(
    mut socket: WebSocket,
    ready: ApplicationEvent,
    mut receiver: broadcast::Receiver<rambledesk_core::ApplicationInvalidation>,
    _permit: OwnedSemaphorePermit,
) {
    if send_event(&mut socket, &ready).await.is_err() {
        return;
    }
    loop {
        tokio::select! {
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
    if request.headers().get(header::HOST).and_then(header_text)
        != Some(state.config.allowed_host.as_str())
        || request.headers().get(header::ORIGIN).and_then(header_text)
            != Some(state.config.allowed_origin.as_str())
    {
        return StatusCode::FORBIDDEN.into_response();
    }

    let websocket = request.uri().path().ends_with("/events");
    let credential = if websocket {
        websocket_credential(request.headers().get(header::SEC_WEBSOCKET_PROTOCOL))
    } else {
        bearer_credential(request.headers().get(header::AUTHORIZATION))
    };
    if !credential.is_some_and(|token| state.authenticator.authenticate(token)) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    next.run(request).await
}

fn header_text(value: &HeaderValue) -> Option<&str> {
    value.to_str().ok()
}

fn bearer_credential(value: Option<&HeaderValue>) -> Option<&str> {
    value
        .and_then(header_text)
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
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
    use super::*;

    #[test]
    fn route_config_requires_same_origin_and_a_connection_budget() {
        assert!(WebAccessRouteConfig::new("127.0.0.1:4000", "http://127.0.0.1:4000", 4).is_ok());
        assert!(WebAccessRouteConfig::new("127.0.0.1:4000", "http://localhost:4000", 4).is_err());
        assert!(WebAccessRouteConfig::new("127.0.0.1:4000", "http://127.0.0.1:4000", 0).is_err());
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
}
