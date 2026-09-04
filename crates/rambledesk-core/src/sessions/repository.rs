use async_trait::async_trait;
use thiserror::Error;

use super::{AgentConfig, NewManagedSession, SessionRecord};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SessionRepositoryError {
    #[error("session was not found")]
    SessionNotFound,
    #[error("agent configuration was not found")]
    AgentConfigNotFound,
    #[error("agent configuration is referenced by a session")]
    AgentConfigInUse,
    #[error("agent configuration is disabled")]
    AgentConfigDisabled,
    #[error("session or agent configuration input is invalid")]
    InvalidInput,
    #[error("session identity or configuration conflicts with stored data")]
    Conflict,
    #[error("stored session data is invalid")]
    CorruptData,
    #[error("session storage operation failed")]
    Storage,
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn save_agent_config(
        &self,
        config: AgentConfig,
    ) -> Result<AgentConfig, SessionRepositoryError>;

    async fn get_agent_config(&self, id: &str) -> Result<AgentConfig, SessionRepositoryError>;

    async fn list_agent_configs(&self) -> Result<Vec<AgentConfig>, SessionRepositoryError>;

    /// A referenced configuration must be disabled or edited instead of removed.
    async fn delete_agent_config(&self, id: &str) -> Result<(), SessionRepositoryError>;

    /// Inserts the local session before any agent is launched. Feedback correlation
    /// is assigned from this local identity, never chosen by the agent.
    async fn create_managed_session(
        &self,
        session: NewManagedSession,
    ) -> Result<SessionRecord, SessionRepositoryError>;

    async fn get_session(&self, session_id: &str) -> Result<SessionRecord, SessionRepositoryError>;

    async fn list_managed_sessions(&self) -> Result<Vec<SessionRecord>, SessionRepositoryError>;

    /// Binds once, or accepts the same remote id again. A different remote id must
    /// not silently replace the original agent context during recovery.
    async fn bind_remote_session(
        &self,
        session_id: &str,
        remote_session_id: &str,
        now: &str,
    ) -> Result<SessionRecord, SessionRepositoryError>;
}
