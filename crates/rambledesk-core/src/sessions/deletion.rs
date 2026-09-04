use async_trait::async_trait;

use super::SessionRepositoryError;
use super::*;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedManagedSession {
    pub session_id: String,
    pub host_id: String,
    pub host_session_id: String,
    pub request_ids: Vec<String>,
}

#[async_trait]
pub trait SessionDeletionRepository: Send + Sync {
    /// Persist before revoking access, stopping owned resources or cleaning files.
    /// Repeated requests retain the first intent; external sessions are rejected.
    async fn begin_managed_session_deletion(
        &self,
        session_id: &str,
        now: &str,
    ) -> Result<(), SessionRepositoryError>;

    async fn is_managed_session_deleting(
        &self,
        session_id: &str,
    ) -> Result<bool, SessionRepositoryError>;

    async fn list_managed_session_deletions(&self) -> Result<Vec<String>, SessionRepositoryError>;

    /// Requires an existing deletion intent and already stopped/revoked runtime.
    /// Files are removed before their database metadata; failures retain the intent
    /// and record so a later call can finish cleanup. Missing files are idempotent.
    async fn delete_managed_session_data(
        &self,
        session_id: &str,
    ) -> Result<DeletedManagedSession, SessionRepositoryError>;
}

impl SessionApplication {
    pub fn with_deletions(mut self, repository: Arc<dyn SessionDeletionRepository>) -> Self {
        self.deletions = Some(repository);
        self
    }

    pub(super) async fn require_workable(&self, session_id: &str) -> Result<(), SessionError> {
        self.managed_record(session_id).await?;
        if let Some(repository) = &self.deletions
            && repository.is_managed_session_deleting(session_id).await?
        {
            return Err(SessionError::NotConnected);
        }
        Ok(())
    }

    pub async fn delete_managed_session(
        &self,
        input: ManagedSessionInput,
    ) -> Result<(), SessionError> {
        let repository = self.deletions.as_ref().ok_or(SessionError::InvalidInput)?;
        match self.managed_record(&input.session_id).await {
            Err(SessionError::Repository(SessionRepositoryError::SessionNotFound)) => return Ok(()),
            result => {
                result?;
            }
        }
        let entry = self.entry(&input.session_id).await;
        entry
            .interrupt
            .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
        let _lifecycle = entry.lifecycle.lock().await;
        if matches!(
            self.managed_record(&input.session_id).await,
            Err(SessionError::Repository(
                SessionRepositoryError::SessionNotFound
            ))
        ) {
            return Ok(());
        }
        repository
            .begin_managed_session_deletion(&input.session_id, &self.clock.now_rfc3339())
            .await?;
        let mut live = entry.live.lock().await;
        live.runtime.connection = SessionConnectionState::Disconnected;
        live.permissions.clear();
        let connection = live.connection.clone();
        drop(live);
        self.session_changed(&input.session_id);
        let result = async {
            let revoked = match &self.feedback {
                Some(provider) => provider.revoke(&input.session_id).await,
                None => Ok(()),
            };
            let stopped = match connection {
                Some(connection) => connection.stop().await,
                None => Ok(()),
            };
            revoked?;
            stopped?;
            if let Some(deliveries) = &self.deliveries {
                deliveries
                    .discard_session_deliveries(&input.session_id, &self.clock.now_rfc3339())
                    .await?;
            }
            repository
                .delete_managed_session_data(&input.session_id)
                .await?;
            Ok::<_, SessionError>(())
        }
        .await;
        if let Err(error) = result {
            let mut live = entry.live.lock().await;
            live.runtime.last_error = Some(error.to_string());
            live.runtime.activity = SessionActivityState::Idle;
            drop(live);
            self.session_changed(&input.session_id);
            return Err(error);
        }
        self.entries.lock().await.remove(&input.session_id);
        self.changed();
        Ok(())
    }
}
