use rambledesk_acp_client::{
    AskField, AskFieldKind, AskOption, AskQuestion, CapabilitySnapshot, LaunchProfileRef,
    PermissionOption as AcpPermissionOption, PermissionRequest, PreflightReport,
};
use rambledesk_core::kernel::{AccessMode, SessionId};
use serde_json::json;

use super::*;
use crate::acp_workbench::model::PermissionTone;

#[test]
fn permission_projection_preserves_raw_tool_call_and_agent_options() {
    let raw = json!({
        "toolCallId": "call-1",
        "title": "Run tests",
        "kind": "execute",
        "rawInput": {"command": "cargo test", "path": "/workspace"}
    });
    let projected = project_permission(
        &PermissionRequest {
            live_request_id: "permission-1".to_owned(),
            session_id: SessionId::new("session-1"),
            tool_call: raw.clone(),
            request_meta: json!({
                "permission": {
                    "title": "Approve project tests",
                    "description": "Tests execute code from this workspace"
                }
            }),
            options: vec![
                AcpPermissionOption {
                    option_id: "allow-once".to_owned(),
                    name: "Allow once".to_owned(),
                    kind: "allow_once".to_owned(),
                },
                AcpPermissionOption {
                    option_id: "reject-once".to_owned(),
                    name: "Reject".to_owned(),
                    kind: "reject_once".to_owned(),
                },
            ],
            queue_position: 0,
        },
        "2026-08-30T10:00:00Z",
    );

    let AttentionItem::Permission {
        title,
        description,
        tool_call,
        command,
        path,
        options,
        ..
    } = projected.item
    else {
        panic!("expected Permission projection")
    };
    assert_eq!(title, "Approve project tests");
    assert_eq!(description, "Tests execute code from this workspace");
    assert_eq!(tool_call, raw);
    assert_eq!(command.as_deref(), Some("cargo test"));
    assert_eq!(path.as_deref(), Some("/workspace"));
    assert_eq!(options[0].tone, PermissionTone::Allow);
    assert_eq!(options[1].tone, PermissionTone::Deny);
}

#[test]
fn exactly_one_select_question_round_trips_opaque_choice_values() {
    let projected = project_question(
        &AskQuestion {
            live_request_id: "question-1".to_owned(),
            session_id: SessionId::new("session-1"),
            tool_call_id: None,
            message: "Choose strategies".to_owned(),
            fields: vec![AskField {
                field_id: "strategies".to_owned(),
                title: "Strategies".to_owned(),
                description: None,
                kind: AskFieldKind::MultiSelect,
                required: true,
                secret: false,
                options: vec![
                    AskOption {
                        label: "Safe".to_owned(),
                        value: json!({"mode": "safe"}),
                    },
                    AskOption {
                        label: "Fast".to_owned(),
                        value: json!(2),
                    },
                ],
            }],
            queue_position: 0,
        },
        "2026-08-30T10:00:00Z",
    );
    let answer = projected
        .binding
        .answer(QuestionAnswerInput {
            request_id: "question-1".to_owned(),
            choice_ids: vec!["choice-0".to_owned(), "choice-1".to_owned()],
            skipped: false,
        })
        .expect("map selected choices");

    assert_eq!(answer.action, rambledesk_acp_client::QuestionAction::Accept);
    assert_eq!(
        answer.content,
        Some(json!({"strategies":[{"mode":"safe"}, 2]}))
    );
}

#[test]
fn live_projection_tracks_fifo_attention_and_disconnect_clears_it() {
    let projection = ProjectionStore::new(Vec::new());
    let permission = PermissionRequest {
        live_request_id: "permission-live".to_owned(),
        session_id: SessionId::new("session-live"),
        tool_call: json!({"title":"Write file"}),
        request_meta: Value::Null,
        options: vec![AcpPermissionOption {
            option_id: "allow-once".to_owned(),
            name: "Allow once".to_owned(),
            kind: "allow_once".to_owned(),
        }],
        queue_position: 0,
    };
    projection.apply_event(LiveSessionEvent::PermissionQueued {
        request: permission,
    });

    let waiting = projection.snapshot();
    assert_eq!(waiting.running_session_ids, ["session-live"]);
    assert_eq!(waiting.attention_items.len(), 1);

    projection.apply_event(LiveSessionEvent::Disconnected {
        session_id: SessionId::new("session-live"),
        reason: "agent exited".to_owned(),
    });
    let disconnected = projection.snapshot();
    assert!(disconnected.running_session_ids.is_empty());
    assert!(disconnected.attention_items.is_empty());
}

#[test]
fn unsupported_question_shape_remains_visible_and_only_allows_decline() {
    let projected = project_question(
        &AskQuestion {
            live_request_id: "question-2".to_owned(),
            session_id: SessionId::new("session-1"),
            tool_call_id: None,
            message: "Tell me more".to_owned(),
            fields: vec![AskField {
                field_id: "details".to_owned(),
                title: "Details".to_owned(),
                description: None,
                kind: AskFieldKind::Text,
                required: true,
                secret: false,
                options: Vec::new(),
            }],
            queue_position: 0,
        },
        "2026-08-30T10:00:00Z",
    );
    let AttentionItem::Question {
        prompt,
        choices,
        allow_skip,
        ..
    } = &projected.item
    else {
        panic!("expected Question projection")
    };
    assert!(prompt.contains("not supported"));
    assert!(choices.is_empty());
    assert!(*allow_skip);
    let answer = projected
        .binding
        .answer(QuestionAnswerInput {
            request_id: "question-2".to_owned(),
            choice_ids: Vec::new(),
            skipped: true,
        })
        .expect("unsupported question can be declined");
    assert_eq!(
        answer.action,
        rambledesk_acp_client::QuestionAction::Decline
    );
}

#[test]
fn preflight_maps_agent_categories_and_reports_unexposed_options() {
    let report = PreflightReport {
        profile_ref: LaunchProfileRef {
            agent_profile_id: "codex".to_owned(),
            launch_profile_id: "codex-acp-npx".to_owned(),
        },
        available: true,
        agent_version: Some("codex-acp 1.7.0".to_owned()),
        capabilities: CapabilitySnapshot {
            protocol_version: 1,
            load_session: true,
            resume_session: true,
            close_session: false,
            mcp_http: true,
            elicitation_form: true,
            raw_agent_capabilities: json!({}),
        },
        supported_access_modes: vec![AccessMode::WorkspaceWrite, AccessMode::Yolo],
        config_options: vec![
            select_option("model", "model", "gpt-5", &["gpt-4", "gpt-5"]),
            select_option(
                "reasoning_effort",
                "thought_level",
                "high",
                &["low", "high"],
            ),
            select_option(
                "mode",
                "mode",
                "agent",
                &["read-only", "agent", "agent-full-access"],
            ),
            json!({"id":"fast-mode","type":"boolean","currentValue":false}),
        ],
        warnings: Vec::new(),
    };

    let mapped = project_preflight("codex", &report);
    assert_eq!(mapped.models, ["gpt-5", "gpt-4"]);
    assert_eq!(mapped.reasoning_efforts, ["high", "low"]);
    assert_eq!(
        mapped.access_modes,
        [AccessMode::WorkspaceWrite, AccessMode::Yolo]
    );
    assert!(mapped.warning.as_deref().unwrap().contains("agent"));
    assert!(mapped.warning.as_deref().unwrap().contains("fast-mode"));
}

fn select_option(id: &str, category: &str, current: &str, values: &[&str]) -> serde_json::Value {
    json!({
        "id": id,
        "category": category,
        "type": "select",
        "currentValue": current,
        "options": values.iter().map(|value| json!({"value":value,"name":value})).collect::<Vec<_>>()
    })
}
