use super::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
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
        repository
            .recover_interrupted_deliveries(&self.clock.now_rfc3339())
            .await?;
        let app = self.clone();
        *worker = Some(tokio::spawn(async move {
            while !app.closing.load(Ordering::SeqCst) {
                if let Err(error) = app.deliver_pending_feedback().await {
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
                }
            }
        }));
        self.changed();
        Ok(())
    }

    async fn deliver_pending_feedback(&self) -> Result<(), SessionError> {
        self.reconcile_closed_sessions().await?;
        let repository = self.deliveries.as_ref().ok_or(SessionError::InvalidInput)?;
        for delivery in repository.list_pending_deliveries().await? {
            if self.closing.load(Ordering::SeqCst) {
                break;
            }
            // A finished prompt may still be retrying its durable completion.
            // Do not cross either that sending attempt or an uncertain outcome.
            if repository
                .list_session_deliveries(&delivery.session_id)
                .await?
                .iter()
                .any(|item| {
                    matches!(
                        item.state,
                        FeedbackDeliveryState::Sending | FeedbackDeliveryState::Uncertain
                    )
                })
            {
                continue;
            }
            let input = SendManagedPromptInput {
                session_id: delivery.session_id.clone(),
                text: format!(
                    "RambleDesk human feedback is ready for request {} (resolution: {}). Call get_feedback with this request_id to retrieve the durable result and its feedback files, then continue the original task in this same Agent session. This is a continuation of your existing task, not a new task.",
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
        result: &Result<String, AgentDriverError>,
    ) -> Result<(), SessionError> {
        if let (Some(repository), Some((request, attempt))) = (&self.deliveries, delivery) {
            let (state, error) = match result {
                Ok(reason) if reason == "EndTurn" => (FeedbackDeliveryState::Delivered, None),
                _ => (
                    FeedbackDeliveryState::Uncertain,
                    Some(
                        "Continuation did not finish normally; inspect the original session before choosing to send again",
                    ),
                ),
            };
            let completed = repository
                .finish_delivery(&request, &attempt, state, error, &self.clock.now_rfc3339())
                .await;
            if matches!(completed, Err(SessionRepositoryError::Storage)) {
                let app = self.clone();
                let repository = repository.clone();
                tokio::spawn(async move {
                    // Retry only the known result of this exact attempt. No prompt
                    // is sent again, and a discarded/replaced attempt ends the loop.
                    while !app.closing.load(Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(250)).await;
                        if app.closing.load(Ordering::SeqCst) {
                            break;
                        }
                        match repository
                            .finish_delivery(
                                &request,
                                &attempt,
                                state,
                                error,
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
