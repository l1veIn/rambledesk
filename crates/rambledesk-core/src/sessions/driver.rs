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
    pub observer: Arc<dyn AgentSessionObserver>,
    pub feedback: Option<super::ManagedFeedbackEndpoint>,
}

pub enum AgentSessionEvent {
    ConfigurationChanged,
    PermissionRequested(super::SessionPermission),
    Activity {
        kind: super::SessionActivityKind,
        text: String,
        tool_call_id: Option<String>,
        append: bool,
    },
    MessageChunk {
        kind: super::SessionActivityKind,
        block: super::SessionContentBlock,
        truncated: bool,
    },
    ToolCall {
        tool_call_id: String,
        patch: super::SessionToolCallPatch,
    },
}

#[async_trait]
pub trait AgentSessionObserver: Send + Sync {
    async fn observe(&self, event: AgentSessionEvent) -> Result<(), AgentDriverError>;
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
    fn configuration(&self) -> super::SessionConfiguration {
        super::SessionConfiguration::default()
    }
    async fn set_configuration(
        &self,
        _: super::SessionConfigChange,
    ) -> Result<(), AgentDriverError> {
        Err(AgentDriverError::new(
            "Agent does not support session configuration",
        ))
    }
    fn is_closed(&self) -> bool;
    async fn prompt(&self, text: &str) -> Result<String, AgentDriverError>;
    async fn prompt_content(
        &self,
        blocks: &[super::SessionPromptContent],
    ) -> Result<String, AgentDriverError> {
        super::validate_prompt_content(blocks)?;
        let mut text = String::new();
        for block in blocks {
            let super::SessionPromptContent::Text { text: part } = block else {
                return Err(AgentDriverError::new(
                    "Agent does not support this prompt content",
                ));
            };
            text.push_str(part);
        }
        self.prompt(&text).await
    }
    async fn cancel(&self) -> Result<(), AgentDriverError>;
    async fn respond_permission(
        &self,
        request_id: &str,
        option_id: Option<&str>,
    ) -> Result<(), AgentDriverError>;
    async fn stop(&self) -> Result<(), AgentDriverError>;
}
