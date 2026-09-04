use super::{AgentConfig, AgentSessionCapabilities, SessionRecord};
use async_trait::async_trait;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
#[error("{message}")]
pub struct AgentDriverError {
    pub message: String,
}

impl AgentDriverError {
    /// Implementations supply safe diagnostics, never raw protocol/stderr data.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub struct AgentSessionLaunch {
    pub config: AgentConfig,
    pub session: SessionRecord,
}

pub struct StartedAgentSession {
    pub connection: Arc<dyn AgentSessionConnection>,
    pub remote_session_id: String,
    pub capabilities: AgentSessionCapabilities,
}

#[async_trait]
pub trait AgentSessionDriver: Send + Sync {
    async fn start(
        &self,
        launch: AgentSessionLaunch,
    ) -> Result<StartedAgentSession, AgentDriverError>;
    /// Checks launch + handshake only and cleans all resources. Does not create a conversation.
    async fn check(
        &self,
        config: &AgentConfig,
    ) -> Result<AgentSessionCapabilities, AgentDriverError>;
}

#[async_trait]
pub trait AgentSessionConnection: Send + Sync {
    fn is_closed(&self) -> bool;
    async fn stop(&self) -> Result<(), AgentDriverError>;
}
