use rambledesk_core::{FeedbackApplication, FeedbackStatus};
use rambledesk_hosts::{
    ContinuationPayload, ContinuationReason, ContinuationResult, ContinuationRouter, ResumePrompt,
};
use tauri::{Emitter, Manager};

use super::RESUME_PROMPT_EVENT;

pub(super) async fn deliver_continuation_after_terminal(
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
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.set_focus();
    }
    if let Err(error) = app.emit(RESUME_PROMPT_EVENT, prompt) {
        tracing::warn!(%error, "failed to emit resume prompt event");
    }
}
