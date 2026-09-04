use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use async_trait::async_trait;
use axum::{
    Router,
    body::Body,
    extract::State,
    http::{
        HeaderMap, Request, StatusCode,
        header::{AUTHORIZATION, HOST, ORIGIN},
    },
    response::IntoResponse,
};
use rambledesk_core::{
    AgentDriverError, FeedbackApplication, ManagedFeedbackEndpoint, ManagedFeedbackProvider,
    ManagedFeedbackScope, SessionRecord,
};
use rambledesk_mcp::{ManagedMcpScope, ManagedRambleDeskMcp};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService,
    session::{SessionManager, local::LocalSessionManager},
};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tower_service::Service;

use crate::{AccessToken, MAX_ATTACHMENT_REQUEST_BODY_BYTES, host_is_loopback, web_security};

pub const MANAGED_MCP_PATH: &str = "/mcp-managed";

type ManagedService = StreamableHttpService<ManagedRambleDeskMcp, LocalSessionManager>;

struct Binding {
    token: AccessToken,
    scope: Arc<ManagedMcpScope>,
    service: ManagedService,
    sessions: Arc<LocalSessionManager>,
    active: RwLock<bool>,
    cancellation: CancellationToken,
}

impl Binding {
    async fn revoke(&self) {
        // Wake admitted HTTP requests first, including clients that never finish
        // sending initialize's body. Their read leases cannot stall revocation.
        self.cancellation.cancel();
        // An HTTP request may already have authenticated when revoke removes the
        // binding. Serialize transport creation with closure so no late initialize
        // can leave a new worker behind after the owned manager is drained.
        let mut active = self.active.write().await;
        *active = false;
        // Tool operations admitted before revocation still finish before delete.
        self.scope.revoke().await;
        let ids: Vec<_> = self
            .sessions
            .sessions
            .read()
            .await
            .keys()
            .cloned()
            .collect();
        for id in ids {
            // rmcp's HTTP cancellation token closes SSE streams, not the session
            // worker. Remove every owned handle and explicitly close its worker.
            let _ = self.sessions.close_session(&id).await;
        }
    }
}

#[derive(Clone)]
struct ListenerState {
    address: SocketAddr,
    allowed_origins: Vec<String>,
    cancellation: CancellationToken,
}

/// Session-scoped capabilities served by the existing authenticated loopback
/// listener. Credentials and endpoint bindings deliberately have no Debug impl.
pub struct LocalManagedFeedbackProvider {
    application: FeedbackApplication,
    mutations: Mutex<()>,
    bindings: RwLock<HashMap<String, Arc<Binding>>>,
    listener: RwLock<Option<ListenerState>>,
}

impl LocalManagedFeedbackProvider {
    pub fn new(application: FeedbackApplication) -> Self {
        Self {
            application,
            mutations: Mutex::new(()),
            bindings: RwLock::new(HashMap::new()),
            listener: RwLock::new(None),
        }
    }

    pub(crate) async fn configure(
        &self,
        address: SocketAddr,
        allowed_origins: Vec<String>,
        cancellation: CancellationToken,
    ) -> Result<(), crate::ServerError> {
        let _mutation = self.mutations.lock().await;
        if self.listener.read().await.is_some() {
            return Err(crate::ServerError::ManagedFeedbackAlreadyBound);
        }
        *self.listener.write().await = Some(ListenerState {
            address,
            allowed_origins,
            cancellation,
        });
        Ok(())
    }

    async fn drain_bindings(&self) {
        let bindings = std::mem::take(&mut *self.bindings.write().await);
        for binding in bindings.into_values() {
            binding.revoke().await;
        }
    }

    pub(crate) async fn shutdown(&self) {
        let _mutation = self.mutations.lock().await;
        *self.listener.write().await = None;
        self.drain_bindings().await;
    }

    async fn authenticate(&self, headers: &HeaderMap) -> Result<Arc<Binding>, StatusCode> {
        if !host_is_loopback(headers.get(HOST)) {
            return Err(StatusCode::FORBIDDEN);
        }
        let listener = self.listener.read().await;
        let Some(listener) = listener
            .as_ref()
            .filter(|listener| !listener.cancellation.is_cancelled())
        else {
            return Err(StatusCode::UNAUTHORIZED);
        };
        if let Some(origin) = headers.get(ORIGIN) {
            let origin = web_security::header_text(origin).ok_or(StatusCode::FORBIDDEN)?;
            if !listener
                .allowed_origins
                .iter()
                .any(|allowed| allowed == origin)
            {
                return Err(StatusCode::FORBIDDEN);
            }
        }
        let credential = web_security::bearer_credential(headers.get(AUTHORIZATION))
            .ok_or(StatusCode::UNAUTHORIZED)?;
        self.bindings
            .read()
            .await
            .values()
            .find(|binding| {
                web_security::constant_time_bytes_eq(
                    binding.token.secret().as_bytes(),
                    credential.as_bytes(),
                )
            })
            .cloned()
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

#[async_trait]
impl ManagedFeedbackProvider for LocalManagedFeedbackProvider {
    async fn bind(
        &self,
        session: &SessionRecord,
    ) -> Result<ManagedFeedbackEndpoint, AgentDriverError> {
        let identity = ManagedFeedbackScope::from_session(session)?;
        let _mutation = self.mutations.lock().await;
        let previous = self.bindings.write().await.remove(&session.session_id);
        if let Some(previous) = previous {
            previous.revoke().await;
        }
        let listener = self
            .listener
            .read()
            .await
            .clone()
            .filter(|listener| !listener.cancellation.is_cancelled())
            .ok_or_else(|| AgentDriverError::new("Managed feedback listener is unavailable"))?;
        let token = AccessToken::generate();
        let scope = Arc::new(ManagedMcpScope::new(identity));
        let cancellation = listener.cancellation.child_token();
        let application = self.application.clone();
        let handler_scope = scope.clone();
        // A session manager is owned by exactly one binding. A valid bearer for
        // another scope can never select this scope's handler by MCP-Session-Id.
        let mut sessions = LocalSessionManager::default();
        // Human feedback commonly takes longer than rmcp's default five-minute
        // idle limit. This private manager lives until its Agent instance is
        // revoked; Binding::revoke explicitly closes all of its transports.
        sessions.session_config.keep_alive = None;
        let sessions = Arc::new(sessions);
        let service = ManagedService::new(
            move || {
                Ok(ManagedRambleDeskMcp::new(
                    application.clone(),
                    handler_scope.clone(),
                ))
            },
            sessions.clone(),
            StreamableHttpServerConfig::default()
                .with_legacy_session_mode(true)
                .with_allowed_origins(listener.allowed_origins)
                .with_max_request_body_bytes(MAX_ATTACHMENT_REQUEST_BODY_BYTES)
                .with_cancellation_token(cancellation.clone()),
        );
        let endpoint = ManagedFeedbackEndpoint {
            url: format!("http://{}{MANAGED_MCP_PATH}", listener.address),
            bearer_token: token.secret().to_owned(),
        };
        self.bindings.write().await.insert(
            session.session_id.clone(),
            Arc::new(Binding {
                token,
                scope,
                service,
                sessions,
                active: RwLock::new(true),
                cancellation,
            }),
        );
        Ok(endpoint)
    }

    async fn revoke(&self, session_id: &str) -> Result<(), AgentDriverError> {
        let _mutation = self.mutations.lock().await;
        let binding = self.bindings.write().await.remove(session_id);
        if let Some(binding) = binding {
            binding.revoke().await;
        }
        Ok(())
    }
}

async fn handle_request(
    State(provider): State<Arc<LocalManagedFeedbackProvider>>,
    request: Request<Body>,
) -> axum::response::Response {
    let binding = match provider.authenticate(request.headers()).await {
        Ok(binding) => binding,
        Err(status) => return status.into_response(),
    };
    let active = binding.active.read().await;
    if !*active {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let response = tokio::select! {
        biased;
        _ = binding.cancellation.cancelled() => return StatusCode::UNAUTHORIZED.into_response(),
        response = binding.service.clone().call(request) => response,
    };
    match response {
        Ok(response) => response.into_response(),
        Err(infallible) => match infallible {},
    }
}

pub(crate) fn managed_router(provider: Arc<LocalManagedFeedbackProvider>) -> Router {
    Router::new().nest(
        MANAGED_MCP_PATH,
        Router::new().fallback(handle_request).with_state(provider),
    )
}

#[cfg(test)]
#[path = "managed_feedback_tests.rs"]
mod tests;
