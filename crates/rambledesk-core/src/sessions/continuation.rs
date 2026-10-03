use super::*;
use crate::ApplicationResourceKey;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tokio::task::JoinSet;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ResolveFeedbackDeliveryInput {
    pub session_id: String,
    pub request_id: String,
    pub action: ResolveDeliveryAction,
}

impl SessionApplication {
    pub fn with_deliveries(mut self, repository: Arc<dyn FeedbackDeliveryRepository>) -> Self {
        self.deliveries = Some(repository);
        self
    }

    /// One runtime owner starts the worker after composing its repositories and listener.
    /// Recovered sending attempts require an explicit user decision; they are not replayed.
    pub async fn start_delivery_worker(&self) -> Result<(), SessionError> {
        self.recover_runtime().await?;
        let mut worker = self.delivery_worker.lock().await;
        if worker.is_some() {
            return Ok(());
        }
        let repository = self.deliveries.as_ref().ok_or(SessionError::InvalidInput)?;
        let recovered_count = repository
            .recover_interrupted_deliveries(&self.clock.now_rfc3339())
            .await?;
        if recovered_count > 0 {
            tracing::info!(
                recovered_count,
                "feedback delivery records reconciled after recovery"
            );
        }
        let app = self.clone();
        *worker = Some(tokio::spawn(async move {
            let mut attempted_starts = HashSet::new();
            let mut starting_sessions = HashSet::new();
            let mut starts = JoinSet::new();
            while !app.closing.load(Ordering::SeqCst) {
                if let Err(error) = app
                    .deliver_pending_feedback(
                        &mut attempted_starts,
                        &mut starting_sessions,
                        &mut starts,
                    )
                    .await
                {
                    // Keep a safe, visible diagnostic on affected live sessions. The
                    // durable queue remains authoritative and the next pass can retry reads.
                    let entries = app.entries.lock().await.clone();
                    for (id, entry) in entries {
                        entry.live.lock().await.runtime.last_error = Some(error.to_string());
                        app.session_changed(&id);
                    }
                }
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(250)) => {},
                    _ = app.delivery_wake.notified() => {},
                    result = starts.join_next(), if !starts.is_empty() => {
                        if let Some(Ok(session_id)) = result {
                            starting_sessions.remove(&session_id);
                        }
                    },
                }
            }
            // Shutdown already advanced every entry's interrupt epoch. Join the
            // canonical owners so no launch outlives application cleanup.
            while starts.join_next().await.is_some() {}
        }));
        self.changed(vec![ApplicationResourceKey::All]);
        Ok(())
    }

    async fn deliver_pending_feedback(
        &self,
        attempted_starts: &mut HashSet<String>,
        starting_sessions: &mut HashSet<String>,
        starts: &mut JoinSet<String>,
    ) -> Result<(), SessionError> {
        self.reconcile_closed_sessions().await?;
        let repository = self.deliveries.as_ref().ok_or(SessionError::InvalidInput)?;
        let pending = repository.list_pending_deliveries().await?;
        attempted_starts.retain(|request| pending.iter().any(|item| &item.request_id == request));
        for delivery in pending {
            if self.closing.load(Ordering::SeqCst) {
                break;
            }
            // Cancellation closes the feedback request without resuming the Agent.
            // Also guard older/custom repositories that still expose cancelled jobs.
            if delivery.resolution == crate::FeedbackResolution::Cancelled {
                continue;
            }
            // One in-flight send per session. An earlier uncertain outcome stays
            // on that request until the user retries or acknowledges it; it must
            // not hold later submitted feedback.
            if repository
                .list_session_deliveries(&delivery.session_id)
                .await?
                .iter()
                .any(|item| {
                    item.resolution != crate::FeedbackResolution::Cancelled
                        && item.state == FeedbackDeliveryState::Sending
                })
            {
                continue;
            }
            if !self
                .prepare_feedback_connection(&delivery, attempted_starts, starting_sessions, starts)
                .await?
            {
                continue;
            }
            let input = SendManagedPromptInput {
                session_id: delivery.session_id.clone(),
                text: format!(
                    "RambleDesk human feedback is ready for request {} (resolution: {}). Use the built-in RambleDesk feedback command: feedback get --request-id <the request_id above>. Read its durable feedback package and continue the original task in this same Agent session. Follow the managed workflow context for invoking RAMBLEDESK_COMMAND. Do not create a replacement request or use external feedback tools.",
                    delivery.request_id,
                    delivery.resolution.as_str()
                ),
            };
            match self.dispatch_prompt(input, Some(delivery)).await {
                Ok(_)
                | Err(
                    SessionError::Busy | SessionError::NotConnected | SessionError::ShuttingDown,
                ) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    async fn prepare_feedback_connection(
        &self,
        delivery: &FeedbackDelivery,
        attempted_starts: &mut HashSet<String>,
        starting_sessions: &mut HashSet<String>,
        starts: &mut JoinSet<String>,
    ) -> Result<bool, SessionError> {
        let session_id = delivery.session_id.as_str();
        let entry = self.entry(session_id).await;
        let epoch = *entry.interrupt.borrow();
        let input = ManagedSessionInput {
            session_id: session_id.into(),
        };
        let snapshot = match self.get_session(input.clone()).await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.record_feedback_start_failure(session_id, epoch, &error)
                    .await;
                return Ok(false);
            }
        };
        if snapshot.deleting
            || snapshot.session.is_prepared()
            || snapshot.runtime.activity != SessionActivityState::Idle
            || !snapshot.interactions.is_empty()
        {
            return Ok(false);
        }
        if snapshot.runtime.connection == SessionConnectionState::Connected {
            return Ok(true);
        }
        if attempted_starts.contains(&delivery.request_id)
            || starting_sessions.contains(session_id)
            || entry
                .stopped_feedback
                .lock()
                .await
                .as_ref()
                .is_none_or(|blocked| blocked.contains(&delivery.request_id))
            || !matches!(
                snapshot.runtime.connection,
                SessionConnectionState::Stopped | SessionConnectionState::Disconnected
            )
            || snapshot.runtime.instance_id.is_some()
            || snapshot.recovery.as_ref().is_some_and(|recovery| {
                recovery.session_id != session_id
                    || recovery.status == SessionRecoveryStatus::Unclosed
                    || recovery.active_turn_id.is_some()
            })
            || matches!(&snapshot.session.management, SessionManagement::Managed { remote_session_id: None, .. }
                if !snapshot.activities.is_empty() || snapshot.recovery.as_ref().is_some_and(|recovery| recovery.status != SessionRecoveryStatus::NeverStarted))
        {
            return Ok(false);
        }
        // Launch each session independently: a slow ACP handshake must not hold
        // another session's connected outbox. Canonical lifecycle/epoch guards
        // still arbitrate explicit starts, Stop, deletion and shutdown.
        // Consume admission only when scheduling: a new submission seen during
        // an older interrupted task's cleanup must remain eligible next pass.
        attempted_starts.insert(delivery.request_id.clone());
        starting_sessions.insert(session_id.to_owned());
        let app = self.clone();
        let request_id = delivery.request_id.clone();
        starts.spawn(async move {
            let session_id = input.session_id.clone();
            if let Err(error) = app.start_feedback_session(input, epoch, request_id).await {
                app.record_feedback_start_failure(&session_id, epoch, &error)
                    .await;
            }
            app.delivery_wake.notify_one();
            session_id
        });
        Ok(false)
    }

    async fn record_feedback_start_failure(
        &self,
        session_id: &str,
        epoch: u64,
        error: &SessionError,
    ) {
        if matches!(
            error,
            SessionError::Interrupted
                | SessionError::NotConnected
                | SessionError::ShuttingDown
                | SessionError::Busy
                | SessionError::NotManaged
                | SessionError::Repository(SessionRepositoryError::SessionNotFound)
        ) {
            return;
        }
        let entry = self.entry(session_id).await;
        let _lifecycle = entry.lifecycle.lock().await;
        let workable = self.require_workable(session_id).await;
        if self.closing.load(Ordering::SeqCst)
            || epoch != *entry.interrupt.borrow()
            || matches!(
                workable,
                Err(SessionError::NotConnected
                    | SessionError::NotManaged
                    | SessionError::Repository(SessionRepositoryError::SessionNotFound))
            )
        {
            return;
        }
        let mut live = entry.live.lock().await;
        // Driver errors already use canonical failure handling. A stale preflight
        // error must never replace a newer connected owner or a retained instance.
        if !matches!(
            live.runtime.connection,
            SessionConnectionState::Stopped | SessionConnectionState::Disconnected
        ) || live.runtime.instance_id.is_some()
            || live.runtime.activity != SessionActivityState::Idle
        {
            return;
        }
        live.runtime.connection = SessionConnectionState::Failed;
        live.runtime.last_error = Some(error.to_string());
        live.runtime.failure = error.agent_failure(AgentFailureStage::Launch);
        drop(live);
        self.session_changed(session_id);
    }

    pub async fn resolve_feedback_delivery(
        &self,
        input: ResolveFeedbackDeliveryInput,
    ) -> Result<ManagedSessionSnapshot, SessionError> {
        self.require_workable(&input.session_id).await?;
        let repository = self.deliveries.as_ref().ok_or(SessionError::InvalidInput)?;
        repository
            .resolve_delivery(
                &input.request_id,
                &input.session_id,
                input.action,
                &self.clock.now_rfc3339(),
            )
            .await?;
        self.session_changed(&input.session_id);
        self.delivery_wake.notify_one();
        self.get_session(ManagedSessionInput {
            session_id: input.session_id,
        })
        .await
    }

    pub(super) async fn finish_feedback_delivery(
        &self,
        delivery: Option<(String, String)>,
    ) -> Result<(), SessionError> {
        if let (Some(repository), Some((request, attempt))) = (&self.deliveries, delivery) {
            // Delivery is the continuation prompt being sent, not the later turn
            // ending. Quota, disconnects and process exits are session problems.
            let completed = repository
                .finish_delivery(
                    &request,
                    &attempt,
                    FeedbackDeliveryState::Delivered,
                    None,
                    &self.clock.now_rfc3339(),
                )
                .await;
            if matches!(completed, Err(SessionRepositoryError::Storage)) {
                let app = self.clone();
                let repository = repository.clone();
                tokio::spawn(async move {
                    // Retry only this attempt's delivered write. No prompt is sent
                    // again, and a discarded/replaced attempt ends the loop.
                    while !app.closing.load(Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(250)).await;
                        if app.closing.load(Ordering::SeqCst) {
                            break;
                        }
                        match repository
                            .finish_delivery(
                                &request,
                                &attempt,
                                FeedbackDeliveryState::Delivered,
                                None,
                                &app.clock.now_rfc3339(),
                            )
                            .await
                        {
                            Ok(delivery) => {
                                app.session_changed(&delivery.session_id);
                                app.delivery_wake.notify_one();
                                break;
                            }
                            Err(SessionRepositoryError::Storage) => continue,
                            Err(_) => break,
                        }
                    }
                });
            }
            completed?;
        }
        Ok(())
    }
}
