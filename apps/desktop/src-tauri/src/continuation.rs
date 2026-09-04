use async_trait::async_trait;
use rambledesk_core::{
    FeedbackApplication, FeedbackStatus, TerminalOperation, TerminalOperationEvent,
    TerminalOperationObserver,
};
use rambledesk_hosts::{
    ContinuationPayload, ContinuationReason, ContinuationResult, ContinuationRouter, ResumePrompt,
};
use tauri::Emitter;

use super::{RESUME_PROMPT_EVENT, diagnostics};

#[derive(Clone)]
pub(super) struct DesktopTerminalOperationObserver {
    app: tauri::AppHandle,
    router: ContinuationRouter,
    application: FeedbackApplication,
}

impl DesktopTerminalOperationObserver {
    pub(super) fn new(
        app: tauri::AppHandle,
        router: ContinuationRouter,
        application: FeedbackApplication,
    ) -> Self {
        Self {
            app,
            router,
            application,
        }
    }
}

#[async_trait]
impl TerminalOperationObserver for DesktopTerminalOperationObserver {
    async fn observe(&self, event: &TerminalOperationEvent) {
        match event.operation {
            TerminalOperation::SubmitFeedback => diagnostics::record_event(
                "feedback_submitted",
                Some(&event.request.request_id),
                None,
                Some("ok"),
                None,
                None,
            ),
            TerminalOperation::CancelFeedback => diagnostics::record_event(
                "feedback_cancelled",
                Some(&event.request.request_id),
                None,
                Some("ok"),
                None,
                None,
            ),
            TerminalOperation::ApproveFeedback => {}
        }
        match self
            .application
            .managed_feedback_session(&event.request.request_id)
            .await
        {
            Ok(Some(_)) => return,
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(%error, "continuation attribution could not be verified");
                return;
            }
        }
        deliver_continuation_after_terminal(
            &self.app,
            &self.router,
            &self.application,
            &event.request.request_id,
            event.request.status,
        )
        .await;
    }
}

async fn deliver_continuation_after_terminal(
    app: &tauri::AppHandle,
    router: &ContinuationRouter,
    application: &FeedbackApplication,
    request_id: &str,
    status: FeedbackStatus,
) {
    let Some(reason) = ContinuationReason::from_status(status) else {
        return;
    };
    let (host_id, host_session_id, source_hint) = match application
        .get_feedback_workspace(request_id.to_owned())
        .await
    {
        Ok(workspace) => (
            workspace.request.host_id,
            workspace.request.host_session_id,
            workspace.request.source_hint,
        ),
        Err(error) => {
            tracing::warn!(%request_id, %error, "continuation: workspace lookup failed; using empty host");
            (String::new(), String::new(), None)
        }
    };

    let payload = ContinuationPayload {
        request_id: request_id.to_owned(),
        host_id: host_id.clone(),
        host_session_id,
        source_hint,
        reason,
    };
    match router.continue_after_terminal(&payload) {
        ContinuationResult::NotRequired {
            strategy_id,
            host_id,
        } => {
            tracing::info!(%request_id, %strategy_id, %host_id, "host continuation not required");
        }
        ContinuationResult::HostDelivered {
            strategy_id,
            host_id,
        } => {
            tracing::info!(%request_id, %strategy_id, %host_id, "host continuation delivered");
        }
        ContinuationResult::UserPrompt {
            strategy_id,
            prompt,
        } => {
            tracing::info!(
                %request_id,
                %strategy_id,
                host = %prompt.host_id,
                "manual continuation prompt ready"
            );
            present_resume_prompt(app, &prompt);
        }
    }
}

fn present_resume_prompt(app: &tauri::AppHandle, prompt: &ResumePrompt) {
    // Deliberately do NOT show/unminimize/focus the main window here: the
    // operator may be in a fullscreen game or another foreground app, and the
    // notification + sound (sent from the frontend listener) are enough of a
    // heads-up. Forcing focus would yank them out of whatever they are doing.
    if let Err(error) = app.emit(RESUME_PROMPT_EVENT, prompt) {
        tracing::warn!(%error, "failed to emit resume prompt event");
    }
}
