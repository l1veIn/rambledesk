use rambledesk_acp_client::{
    CapabilitySnapshot, LiveSessionEvent, ManagedSessionSnapshot, RecoveryMethod, RunState,
};
use rambledesk_core::kernel::SessionId;
use serde_json::json;

use super::*;
use crate::acp_workbench::orchestration::real::ProjectionStore;

#[test]
fn repeated_running_does_not_split_the_turn_and_chunks_are_normalized() {
    let mut timeline = TimelineProjection::default();
    timeline.apply_state("session-1", RunState::Running, None);
    timeline.apply_state("session-1", RunState::Running, None);
    timeline.apply_update(
        "session-1",
        &json!({
            "sessionUpdate": "agent_thought_chunk",
            "content": {"type": "text", "text": "Let me "}
        }),
    );
    timeline.apply_update(
        "session-1",
        &json!({
            "sessionUpdate": "agent_thought_chunk",
            "content": {"type": "text", "text": "check."}
        }),
    );
    timeline.apply_update(
        "session-1",
        &json!({
            "sessionUpdate": "agent_message_chunk",
            "content": {"type": "text", "text": "Done."}
        }),
    );

    let snapshot = timeline.snapshot("session-1");
    assert_eq!(snapshot.turns.len(), 1);
    let thoughts = snapshot.turns[0]
        .entries
        .iter()
        .filter(|entry| entry.kind == TimelineEntryKind::Thought)
        .collect::<Vec<_>>();
    assert_eq!(thoughts.len(), 1);
    assert_eq!(thoughts[0].content, "Let me check.");
    assert_eq!(
        snapshot.turns[0]
            .entries
            .iter()
            .filter(|entry| entry.kind == TimelineEntryKind::Message)
            .count(),
        1
    );
}

#[test]
fn tool_updates_and_plans_patch_existing_entries() {
    let mut timeline = TimelineProjection::default();
    timeline.apply_state("session-1", RunState::Running, None);
    timeline.apply_update(
        "session-1",
        &json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "call-1",
            "title": "Run tests",
            "status": "in_progress",
            "rawInput": {"command": "cargo test"}
        }),
    );
    timeline.apply_update(
        "session-1",
        &json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": "call-1",
            "status": "completed",
            "rawOutput": {"text": "all tests passed"}
        }),
    );
    timeline.apply_update(
        "session-1",
        &json!({
            "sessionUpdate": "plan",
            "entries": [
                {"content": "Inspect", "status": "completed"},
                {"content": "Implement", "status": "in_progress"}
            ]
        }),
    );
    timeline.apply_update(
        "session-1",
        &json!({
            "sessionUpdate": "plan",
            "entries": [
                {"content": "Inspect", "status": "completed"},
                {"content": "Implement", "status": "completed"}
            ]
        }),
    );

    let snapshot = timeline.snapshot("session-1");
    let entries = &snapshot.turns[0].entries;
    let tools = entries
        .iter()
        .filter(|entry| entry.kind == TimelineEntryKind::Tool)
        .collect::<Vec<_>>();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].id, "tool:call-1");
    assert_eq!(tools[0].title, "Run tests");
    assert_eq!(tools[0].content, "all tests passed");
    assert_eq!(tools[0].status, TimelineEntryStatus::Completed);
    let plans = entries
        .iter()
        .filter(|entry| entry.title == "Plan")
        .collect::<Vec<_>>();
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].status, TimelineEntryStatus::Completed);
    assert!(plans[0].content.contains("[completed] Implement"));
}

#[test]
fn ready_folds_the_current_turn_and_disconnect_fails_only_the_active_turn() {
    let mut timeline = TimelineProjection::default();
    timeline.apply_state("session-1", RunState::Running, None);
    timeline.apply_update(
        "session-1",
        &json!({
            "sessionUpdate": "agent_message_chunk",
            "content": {"type": "text", "text": "First result"}
        }),
    );
    timeline.apply_state("session-1", RunState::Ready, None);
    timeline.apply_state("session-1", RunState::Running, None);
    timeline.apply_update(
        "session-1",
        &json!({
            "sessionUpdate": "agent_thought_chunk",
            "content": {"type": "text", "text": "unfinished"}
        }),
    );
    timeline.apply_state("session-1", RunState::Disconnected, Some("agent exited"));

    let snapshot = timeline.snapshot("session-1");
    assert_eq!(snapshot.turns.len(), 2);
    assert_eq!(snapshot.turns[0].status, TimelineTurnStatus::Completed);
    assert!(snapshot.turns[0].completed_at.is_some());
    assert_eq!(snapshot.turns[1].status, TimelineTurnStatus::Failed);
    assert!(
        snapshot.turns[1]
            .entries
            .iter()
            .any(|entry| entry.kind == TimelineEntryKind::Error && entry.content == "agent exited")
    );
}

#[test]
fn retention_and_content_are_bounded_without_breaking_utf8() {
    let mut timeline = TimelineProjection::default();
    for turn in 0..(MAX_TIMELINE_TURNS + 3) {
        timeline.apply_state("session-1", RunState::Running, None);
        for call in 0..(MAX_TIMELINE_ENTRIES_PER_TURN + 5) {
            timeline.apply_update(
                "session-1",
                &json!({
                    "sessionUpdate": "tool_call",
                    "toolCallId": format!("{turn}-{call}"),
                    "title": "large",
                    "rawInput": "你".repeat(MAX_TIMELINE_CONTENT_BYTES)
                }),
            );
        }
        timeline.apply_state("session-1", RunState::Ready, None);
    }

    let snapshot = timeline.snapshot("session-1");
    assert_eq!(snapshot.turns.len(), MAX_TIMELINE_TURNS);
    assert!(
        snapshot
            .turns
            .iter()
            .all(|turn| turn.entries.len() <= MAX_TIMELINE_ENTRIES_PER_TURN)
    );
    assert!(
        snapshot
            .turns
            .iter()
            .flat_map(|turn| &turn.entries)
            .all(|entry| entry.content.len() <= MAX_TIMELINE_CONTENT_BYTES
                && entry.content.is_char_boundary(entry.content.len()))
    );
}

#[test]
fn snapshot_refresh_preserves_the_existing_live_timeline() {
    let projection = ProjectionStore::new(Vec::new());
    projection.apply_event(LiveSessionEvent::StateChanged {
        session_id: SessionId::new("session-live"),
        state: RunState::Running,
    });
    projection.apply_event(LiveSessionEvent::SessionUpdate {
        session_id: SessionId::new("session-live"),
        update: json!({
            "sessionUpdate": "agent_message_chunk",
            "content": {"type": "text", "text": "still here"}
        }),
    });

    projection.apply_snapshot(ManagedSessionSnapshot {
        session_id: SessionId::new("session-live"),
        acp_session_id: "acp-live".to_owned(),
        recovery_method: RecoveryMethod::Resume,
        capabilities: CapabilitySnapshot {
            protocol_version: 1,
            load_session: true,
            resume_session: true,
            close_session: false,
            mcp_http: true,
            elicitation_form: true,
            raw_agent_capabilities: json!({}),
        },
        config_options: Vec::new(),
        state: RunState::Running,
        permissions: Vec::new(),
        questions: Vec::new(),
    });

    let snapshot = projection.snapshot();
    assert_eq!(snapshot.timelines.len(), 1);
    assert_eq!(snapshot.timelines[0].turns.len(), 1);
    assert!(
        snapshot.timelines[0].turns[0]
            .entries
            .iter()
            .any(|entry| entry.content == "still here")
    );
}
