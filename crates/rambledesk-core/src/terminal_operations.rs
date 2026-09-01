use std::{collections::HashSet, sync::Arc};

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::{
    ApplicationError, ApproveFeedbackInput, CancelFeedbackInput, FeedbackApplication,
    FeedbackRequestView, FeedbackStatus, SubmitFeedbackInput,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalOperation {
    SubmitFeedback,
    ApproveFeedback,
    CancelFeedback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalOperationEvent {
    pub operation: TerminalOperation,
    pub request: FeedbackRequestView,
}

#[async_trait]
pub trait TerminalOperationObserver: Send + Sync {
    async fn observe(&self, event: &TerminalOperationEvent);
}

#[derive(Debug, Default)]
pub struct NoopTerminalOperationObserver;

#[async_trait]
impl TerminalOperationObserver for NoopTerminalOperationObserver {
    async fn observe(&self, _event: &TerminalOperationEvent) {}
}

/// Workbench-owned terminal operations shared by every Application Transport
/// Implementation. Host Adapters continue to call `FeedbackApplication`
/// directly so an adapter cancellation does not trigger operator continuation.
#[derive(Clone)]
pub struct WorkbenchTerminalOperations {
    application: FeedbackApplication,
    observer: Arc<dyn TerminalOperationObserver>,
    observed_requests: Arc<Mutex<HashSet<String>>>,
}

impl WorkbenchTerminalOperations {
    pub fn new(
        application: FeedbackApplication,
        observer: Arc<dyn TerminalOperationObserver>,
    ) -> Self {
        Self {
            application,
            observer,
            observed_requests: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn without_observer(application: FeedbackApplication) -> Self {
        Self::new(application, Arc::new(NoopTerminalOperationObserver))
    }

    pub async fn submit_feedback(
        &self,
        input: SubmitFeedbackInput,
    ) -> Result<FeedbackRequestView, ApplicationError> {
        let request = self.application.submit_feedback(input).await?;
        self.observe_once(TerminalOperation::SubmitFeedback, &request)
            .await;
        Ok(request)
    }

    pub async fn approve_feedback(
        &self,
        input: ApproveFeedbackInput,
    ) -> Result<FeedbackRequestView, ApplicationError> {
        let request = self.application.approve_feedback(input).await?;
        self.observe_once(TerminalOperation::ApproveFeedback, &request)
            .await;
        Ok(request)
    }

    pub async fn cancel_feedback(
        &self,
        input: CancelFeedbackInput,
    ) -> Result<FeedbackRequestView, ApplicationError> {
        let request = self.application.cancel_feedback(input).await?;
        self.observe_once(TerminalOperation::CancelFeedback, &request)
            .await;
        Ok(request)
    }

    async fn observe_once(&self, operation: TerminalOperation, request: &FeedbackRequestView) {
        if !matches!(
            request.status,
            FeedbackStatus::Completed | FeedbackStatus::Cancelled
        ) {
            return;
        }

        let first_terminal_result = self
            .observed_requests
            .lock()
            .await
            .insert(request.request_id.clone());
        if first_terminal_result {
            self.observer
                .observe(&TerminalOperationEvent {
                    operation,
                    request: request.clone(),
                })
                .await;
        }
    }
}
