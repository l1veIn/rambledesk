use std::collections::HashSet;

use rambledesk_core::kernel::{FeedbackRequestStatus, SessionLifecycle, WorkbenchSnapshot};

use super::model::{
    AcpSessionSummary, AcpWorkbenchSnapshot, AttentionItem, AttentionStatus, SessionStatus,
};
use super::orchestration::LiveAcpProjection;

pub(super) fn project_workbench(
    durable: WorkbenchSnapshot,
    mut live: LiveAcpProjection,
) -> AcpWorkbenchSnapshot {
    for request in &durable.feedback_requests {
        let draft = durable
            .drafts
            .iter()
            .find(|draft| draft.request_id.as_ref() == Some(&request.request_id));
        live.attention_items.push(AttentionItem::Feedback {
            id: request.request_id.to_string(),
            session_id: request.session_id.to_string(),
            title: request.title.clone(),
            created_at: request.created_at.clone(),
            updated_at: request
                .resolved_at
                .clone()
                .unwrap_or_else(|| request.created_at.clone()),
            status: match request.status {
                FeedbackRequestStatus::Waiting => AttentionStatus::Waiting,
                FeedbackRequestStatus::Submitted => AttentionStatus::Submitted,
                FeedbackRequestStatus::Cancelled => AttentionStatus::Cancelled,
            },
            summary: request.instructions.clone(),
            instructions: request.instructions.clone(),
            actions: request
                .actions
                .iter()
                .map(|action| action.instruction.clone())
                .collect(),
            draft_document: draft.and_then(|draft| serde_json::from_str(&draft.document_json).ok()),
            draft_markdown: draft
                .map(|draft| draft.body_markdown.clone())
                .unwrap_or_default(),
            draft_revision: draft.map_or(0, |draft| draft.revision),
        });
    }

    let running = live
        .running_session_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let sessions = durable
        .sessions
        .into_iter()
        .map(|session| {
            let pending_count = live
                .attention_items
                .iter()
                .filter(|item| {
                    item.session_id() == session.session_id.as_str() && item.is_waiting()
                })
                .count() as u32;
            let launch = session.launch_configuration.as_ref();
            let agent_id = launch
                .map(|value| value.agent_profile_id.clone())
                .unwrap_or_default();
            let agent_label = live
                .agents
                .iter()
                .find(|agent| agent.id == agent_id)
                .map(|agent| agent.label.clone())
                .unwrap_or_else(|| agent_id.clone());
            let status = if pending_count > 0 {
                SessionStatus::Waiting
            } else if running.contains(session.session_id.as_str()) {
                SessionStatus::Running
            } else if session.lifecycle == SessionLifecycle::Stopped {
                SessionStatus::Completed
            } else {
                SessionStatus::Offline
            };
            AcpSessionSummary {
                session_id: session.session_id.to_string(),
                title: session.title,
                agent_id,
                agent_label,
                workspace: launch
                    .map(|value| value.workspace_reference.clone())
                    .unwrap_or_default(),
                model: launch
                    .and_then(|value| value.model.clone())
                    .unwrap_or_default(),
                reasoning_effort: launch
                    .and_then(|value| value.reasoning_effort.clone())
                    .unwrap_or_default(),
                access_mode: launch
                    .map(|value| value.access_mode)
                    .unwrap_or(rambledesk_core::kernel::AccessMode::ReadOnly),
                status,
                pending_count,
                pinned_at: session.pinned_at,
                archived_at: session.archived_at,
                updated_at: session.updated_at,
            }
        })
        .collect();
    AcpWorkbenchSnapshot {
        sessions,
        attention_items: live.attention_items,
        agents: live.agents,
        timelines: live.timelines,
    }
}
