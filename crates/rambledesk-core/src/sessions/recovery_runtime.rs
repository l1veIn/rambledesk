use std::sync::Arc;

use super::{application::SessionEntry, *};

impl SessionApplication {
    pub fn with_recovery(mut self, repository: Arc<dyn SessionRecoveryRepository>) -> Self {
        self.recovery = Some(repository);
        self
    }

    /// One application owner reconciles historical checkpoints before any launch.
    /// Reading a recovered session never creates a replacement Agent conversation.
    pub async fn recover_runtime(&self) -> Result<(), SessionError> {
        self.recovery_ready
            .get_or_try_init(|| async {
                if let Some(repository) = &self.recovery {
                    let recovered = repository
                        .recover_open_runs(&self.clock.now_rfc3339())
                        .await?;
                    if !recovered.is_empty() {
                        self.changed();
                    }
                }
                Ok::<_, SessionError>(())
            })
            .await?;
        Ok(())
    }

    pub(super) async fn begin_run(
        &self,
        session_id: &str,
        instance: &str,
    ) -> Result<(), SessionError> {
        if let Some(repository) = &self.recovery {
            repository
                .begin_run(session_id, instance, &self.clock.now_rfc3339())
                .await?;
        }
        Ok(())
    }

    pub(super) async fn begin_turn(
        &self,
        session_id: &str,
        instance: &str,
        turn: &str,
    ) -> Result<(), SessionError> {
        if let Some(repository) = &self.recovery {
            repository
                .begin_turn(session_id, instance, turn, &self.clock.now_rfc3339())
                .await?;
        }
        Ok(())
    }

    pub(super) async fn finish_turn(
        &self,
        session_id: &str,
        instance: &str,
        turn: &str,
    ) -> Result<(), SessionError> {
        if let Some(repository) = &self.recovery {
            repository
                .finish_turn(session_id, instance, turn, &self.clock.now_rfc3339())
                .await?;
        }
        Ok(())
    }

    /// Caller owns lifecycle. Lock order is lifecycle -> events -> live. Neither
    /// event/live guard is held while draining feedback or stopping the process.
    pub(super) async fn retire_entry_locked(
        &self,
        session_id: &str,
        entry: &SessionEntry,
        end: SessionRunEnd,
        diagnostic: Option<&str>,
    ) -> Result<(), SessionError> {
        let events = entry.events.lock().await;
        let mut live = entry.live.lock().await;
        let connection = live.connection.clone();
        let instance = live.runtime.instance_id.clone();
        live.runtime.connection = SessionConnectionState::Disconnected;
        live.runtime.activity = SessionActivityState::Idle;
        live.permissions.clear();
        live.cancelling = false;
        drop(live);
        drop(events);
        let revoked = match &self.feedback {
            Some(provider) => provider
                .revoke(session_id)
                .await
                .map_err(SessionError::from),
            None => Ok(()),
        };
        let stopped = match &connection {
            Some(connection) => connection.stop().await.map_err(SessionError::from),
            None => Ok(()),
        };
        // Keep a failed stop/checkpoint attributable to its original run so the
        // next stop/reconcile can retry it before a replacement is launched.
        let closed = if stopped.is_ok() {
            match (&self.recovery, &instance) {
                (Some(repository), Some(instance)) => repository
                    .close_run(
                        session_id,
                        instance,
                        end,
                        diagnostic,
                        &self.clock.now_rfc3339(),
                    )
                    .await
                    .map(|_| ())
                    .map_err(SessionError::from),
                _ => Ok(()),
            }
        } else {
            Ok(())
        };
        let mut live = entry.live.lock().await;
        if stopped.is_ok() {
            live.connection = None;
        }
        if stopped.is_ok() && closed.is_ok() {
            live.runtime.instance_id = None;
        }
        let result = revoked.and(stopped).and(closed);
        if result.is_ok() && matches!(end, SessionRunEnd::Stopped) {
            live.runtime.connection = SessionConnectionState::Stopped;
        }
        if let Some(diagnostic) = diagnostic {
            live.runtime.last_error = Some(diagnostic.into());
        }
        if let Err(error) = &result {
            live.runtime.last_error = Some(error.to_string());
        }
        drop(live);
        self.session_changed(session_id);
        result
    }

    pub(super) async fn reconcile_closed_entry(
        &self,
        session_id: &str,
        entry: &SessionEntry,
    ) -> Result<(), SessionError> {
        // get_session is also called at the end of start/stop while lifecycle is
        // already held. A best-effort lease avoids re-entering that same lock.
        let Ok(_lifecycle) = entry.lifecycle.try_lock() else {
            return Ok(());
        };
        let live = entry.live.lock().await;
        let closed = live
            .connection
            .as_ref()
            .is_some_and(|connection| connection.is_closed())
            || (live.runtime.instance_id.is_some()
                && live.runtime.connection == SessionConnectionState::Disconnected);
        drop(live);
        if closed {
            self.retire_entry_locked(
                session_id,
                entry,
                SessionRunEnd::Interrupted,
                Some("Agent connection closed; resume the original session to continue"),
            )
            .await?;
        }
        Ok(())
    }

    /// A delayed cancellation watchdog must acquire the lifecycle lease before
    /// comparing identity; it must never interrupt a queued replacement launch.
    pub(super) async fn stop_if_current(
        &self,
        session_id: &str,
        expected_instance: &str,
        expected_turn: &str,
    ) -> Result<bool, SessionError> {
        let Some(entry) = self.entries.lock().await.get(session_id).cloned() else {
            return Ok(false);
        };
        let _lifecycle = entry.lifecycle.lock().await;
        let events = entry.events.lock().await;
        let live = entry.live.lock().await;
        let current = events.turn_id.as_deref() == Some(expected_turn)
            && live.runtime.instance_id.as_deref() == Some(expected_instance)
            && live.runtime.activity != SessionActivityState::Idle
            && live.cancelling;
        drop(live);
        drop(events);
        if !current {
            return Ok(false);
        }
        self.retire_entry_locked(
            session_id,
            &entry,
            SessionRunEnd::Stopped,
            Some("Agent did not finish cancellation; its instance was stopped"),
        )
        .await?;
        Ok(true)
    }

    pub(super) async fn reconcile_closed_sessions(&self) -> Result<(), SessionError> {
        let entries = self.entries.lock().await.clone();
        let mut failure = None;
        for (id, entry) in entries {
            if let Err(error) = self.reconcile_closed_entry(&id, &entry).await
                && !matches!(
                    error,
                    SessionError::Repository(SessionRepositoryError::SessionNotFound)
                )
            {
                failure = Some(error);
            }
        }
        failure.map_or(Ok(()), Err)
    }
}
