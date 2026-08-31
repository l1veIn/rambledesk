use rambledesk_acp_client::{
    AskField, AskFieldKind, AskOption, AskQuestion, CapabilitySnapshot, LaunchConfigKind,
    LaunchConfigOption, LaunchConfigSource, LaunchProfileRef, LaunchSelectOption,
    PermissionOption as AcpPermissionOption, PermissionRequest, PreflightReport,
};
use rambledesk_core::kernel::SessionId;
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
fn preflight_preserves_every_agent_option_in_order() {
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
        schema_digest: "sha256:test-schema".to_owned(),
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
            LaunchConfigOption {
                id: "fast-mode".to_owned(),
                name: "Fast mode".to_owned(),
                description: None,
                category: None,
                source: LaunchConfigSource::Agent,
                kind: LaunchConfigKind::Boolean {
                    current_value: false,
                },
            },
        ],
        warnings: Vec::new(),
    };

    let mapped = project_preflight("codex", &report);
    assert_eq!(mapped.schema_digest, "sha256:test-schema");
    assert_eq!(
        mapped
            .config_options
            .iter()
            .map(|option| option.id.as_str())
            .collect::<Vec<_>>(),
        ["model", "reasoning_effort", "mode", "fast-mode"]
    );
    assert!(matches!(
        mapped.config_options[3].kind,
        LaunchConfigKind::Boolean {
            current_value: false
        }
    ));
    assert!(mapped.warning.is_none());
}

fn select_option(id: &str, category: &str, current: &str, values: &[&str]) -> LaunchConfigOption {
    LaunchConfigOption {
        id: id.to_owned(),
        name: id.to_owned(),
        description: None,
        category: Some(category.to_owned()),
        source: LaunchConfigSource::Agent,
        kind: LaunchConfigKind::Select {
            current_value: current.to_owned(),
            options: values
                .iter()
                .map(|value| LaunchSelectOption {
                    value: (*value).to_owned(),
                    name: (*value).to_owned(),
                    description: None,
                    group: None,
                })
                .collect(),
            groups: Vec::new(),
        },
    }
}
