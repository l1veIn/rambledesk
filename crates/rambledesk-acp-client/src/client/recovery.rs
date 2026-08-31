use std::fmt::Write as _;

use rambledesk_core::kernel::{
    AgentWorkPayload, FeedbackResolution, RambleSubmissionRecord, SessionRecoverySnapshot,
};

const RECOVERY_MARKER: &str = "[RambleDesk Recovery Context]";

pub(super) fn build_recovery_prompt(recovery: &SessionRecoverySnapshot) -> String {
    let mut output = String::from(RECOVERY_MARKER);
    output.push_str(
        "\n\nThe prior ACP Session could not be resumed or loaded. Reconstruct the task from the durable RambleDesk context below. This is recovery context, not new human feedback. Do not call get_feedback during this turn; RambleDesk will send a separate Feedback Resume prompt for every pending delivery. Do not repeat work already completed. End this recovery turn after restoring context.\n",
    );

    let launch = recovery
        .session
        .launch_configuration
        .as_ref()
        .expect("Managed Session recovery requires Launch Configuration");
    let _ = write!(
        output,
        "\nSession\n- title: {}\n- workspace_reference: {}\n- agent_profile_id: {}\n- launch_profile_id: {}\n- model: {}\n- reasoning_effort: {}\n- access_mode: {:?}\n",
        recovery.session.title,
        launch.workspace_reference,
        launch.agent_profile_id,
        launch.launch_profile_id,
        launch.model.as_deref().unwrap_or("agent default"),
        launch
            .reasoning_effort
            .as_deref()
            .unwrap_or("agent default"),
        launch.access_mode,
    );

    if let Some(submission) = &recovery.launch_submission {
        append_submission(&mut output, "Original Launch Ramble", submission);
    }
    for (index, submission) in recovery.steering_submissions.iter().enumerate() {
        append_submission(
            &mut output,
            &format!("Steering Ramble {}", index + 1),
            submission,
        );
    }

    output.push_str("\nPending Feedback Deliveries\n");
    if recovery.pending_feedback.is_empty() {
        output.push_str("- none\n");
    } else {
        for pending in &recovery.pending_feedback {
            let _ = writeln!(
                output,
                "- request_id: {}; delivery_id: {}; resolution: {}; title: {}; instructions: {}",
                pending.request.request_id,
                pending.delivery.delivery_id,
                match pending.delivery.resolution {
                    FeedbackResolution::Submitted => "submitted",
                    FeedbackResolution::Cancelled => "cancelled",
                },
                pending.request.title,
                pending.request.instructions,
            );
            for action in &pending.request.actions {
                let _ = writeln!(
                    output,
                    "  - requested action {}: {}",
                    action.id, action.instruction
                );
            }
        }
    }

    output.push_str("\nPending Agent Work\n");
    if recovery.pending_agent_work.is_empty() {
        output.push_str("- none\n");
    } else {
        for work in &recovery.pending_agent_work {
            let payload = match &work.payload {
                AgentWorkPayload::Launch { .. } => "launch_prompt",
                AgentWorkPayload::Steering { .. } => "steering_prompt",
                AgentWorkPayload::FeedbackResume { .. } => "feedback_resume",
            };
            let _ = writeln!(
                output,
                "- work_id: {}; kind: {}; source_id: {}; prior_attempts: {}",
                work.work_id, payload, work.source_id, work.attempt_count
            );
        }
    }
    output
}

fn append_submission(output: &mut String, heading: &str, submission: &RambleSubmissionRecord) {
    let _ = write!(
        output,
        "\n{heading}\nsubmission_id: {}\n{}\n",
        submission.submission_id, submission.body_markdown
    );
    if !submission.artifacts.is_empty() {
        output.push_str("Artifacts (metadata only; no local paths):\n");
        for artifact in &submission.artifacts {
            let _ = writeln!(
                output,
                "- {} ({}, {} bytes, {})",
                artifact.display_name, artifact.media_type, artifact.size_bytes, artifact.sha256
            );
        }
    }
}
