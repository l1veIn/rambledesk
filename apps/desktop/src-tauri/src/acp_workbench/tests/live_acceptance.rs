use rambledesk_acp_client::{LaunchConfigKind, LaunchConfigSelection};

use super::super::AcpWorkbenchState;
use super::super::model::{
    AttentionItem, DraftInput, FeedbackDecisionInput, LaunchDraftInput, LaunchPreflightInput,
};

/// Real-network acceptance harness for the pinned Agent catalog. It is ignored
/// by normal CI and intentionally selects exactly one Agent per process so ACP
/// credentials, package managers, and process trees are never exercised in
/// parallel.
#[tokio::test]
#[ignore = "requires installed Agent credentials and may download pinned clients"]
async fn live_agent_install_connect_and_optional_ramble() {
    let agent_id = std::env::var("RAMBLEDESK_ACP_TEST_AGENT")
        .expect("set RAMBLEDESK_ACP_TEST_AGENT to a built-in Agent id");
    let mode = std::env::var("RAMBLEDESK_ACP_TEST_MODE").unwrap_or_else(|_| "connect".to_owned());
    let run_id = std::env::var("RAMBLEDESK_ACP_TEST_RUN").unwrap_or_else(|_| "default".to_owned());
    let workspace = std::env::var("RAMBLEDESK_ACP_TEST_WORKSPACE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().expect("current directory"));
    let root = std::env::var("RAMBLEDESK_ACP_TEST_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("rambledesk-acp-acceptance"));
    let state = AcpWorkbenchState::open(crate::config::v3_storage_paths(root))
        .await
        .expect("open live ACP Workbench");
    let existing_session_ids = state
        .read()
        .await
        .expect("read existing live Workbench")
        .sessions
        .into_iter()
        .map(|session| session.session_id)
        .collect::<std::collections::HashSet<_>>();

    let probe = LaunchPreflightInput {
        workspace: workspace.to_string_lossy().into_owned(),
        agent_id: agent_id.clone(),
    };
    let preflight = state.preflight(&probe).await.unwrap_or_else(|error| {
        panic!(
            "live preflight failed: {}",
            serde_json::to_string(&error).expect("serialize error")
        )
    });
    println!(
        "ACP_ACCEPTANCE_PREFLIGHT={}",
        serde_json::to_string(&preflight).expect("serialize preflight")
    );
    if mode != "launch" && mode != "loop" {
        state.shutdown().await;
        return;
    }

    let preferred_access = std::env::var("RAMBLEDESK_ACP_TEST_ACCESS").ok();
    let preferred_model = std::env::var("RAMBLEDESK_ACP_TEST_MODEL").ok();
    let preferred_reasoning = std::env::var("RAMBLEDESK_ACP_TEST_REASONING").ok();
    let config_values = preflight
        .config_options
        .iter()
        .filter_map(|option| match &option.kind {
            LaunchConfigKind::Select {
                current_value,
                options,
                ..
            } => {
                let preferred = match option.category.as_deref() {
                    Some("model") => preferred_model.as_ref(),
                    Some("thought_level") | Some("reasoning_effort") => {
                        preferred_reasoning.as_ref()
                    }
                    Some("permissions") => preferred_access.as_ref(),
                    _ => None,
                };
                let value = preferred
                    .map(|value| serde_json::json!(value))
                    .filter(|value| {
                        options
                            .iter()
                            .any(|candidate| value.as_str() == Some(&candidate.value))
                    })
                    .unwrap_or_else(|| serde_json::json!(current_value));
                Some(LaunchConfigSelection {
                    id: option.id.clone(),
                    value,
                })
            }
            LaunchConfigKind::Boolean { current_value } => Some(LaunchConfigSelection {
                id: option.id.clone(),
                value: serde_json::json!(current_value),
            }),
            LaunchConfigKind::Unsupported { .. } => None,
        })
        .collect();
    let input = LaunchDraftInput {
        submission_id: format!("acceptance-{agent_id}-{run_id}"),
        workspace: probe.workspace,
        agent_id: probe.agent_id,
        schema_digest: preflight.schema_digest,
        config_values,
        document_json: r#"{"type":"doc"}"#.to_owned(),
        body_markdown: "# New Ramble\n\nNo task brief has been provided for this new RambleDesk Session. Before any substantive work, call request_feedback exactly once to ask the human in RambleDesk for their goal, relevant context and materials, constraints, desired output, priorities, and completion criteria. End this turn immediately after request_feedback; RambleDesk will keep the Session open and resume it when the human responds. Do not ask for the task brief in plain chat, guess the task, or start work.".to_owned(),
    };
    let launched = state.launch(input).await.unwrap_or_else(|error| {
        panic!(
            "live launch failed: {}",
            serde_json::to_string(&error).expect("serialize error")
        )
    });
    let session_id = launched
        .sessions
        .iter()
        .find(|session| {
            session.agent_id == agent_id && !existing_session_ids.contains(&session.session_id)
        })
        .expect("launched session")
        .session_id
        .clone();
    let mut first_request_id = None;
    for attempt in 0..18 {
        let snapshot = state.read().await.expect("read live Workbench");
        if let Some(item) = snapshot.attention_items.iter().find(|item| {
            item.session_id() == session_id && matches!(item, AttentionItem::Feedback { .. })
        }) {
            println!(
                "ACP_ACCEPTANCE_RAMBLE={}",
                serde_json::to_string(item).expect("serialize attention item")
            );
            if let AttentionItem::Feedback { id, .. } = item {
                first_request_id = Some(id.clone());
                break;
            }
        }
        println!("ACP_ACCEPTANCE_WAIT={attempt}");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
    let Some(first_request_id) = first_request_id else {
        state.shutdown().await;
        panic!("Agent completed launch without producing a RambleDesk attention request");
    };
    if mode != "loop" {
        state.shutdown().await;
        return;
    }

    const CONTENT_MARKER: &str = "RAMBLE_LOOP_CONTENT_42";
    let task = "Task marker RAMBLE_LOOP_CONTENT_42. Read README.md without modifying any files. Then create a new Feedback Request with a two-sentence summary, include the exact marker RAMBLE_LOOP_CONTENT_42, and ask the human to review it.";
    let document_json = serde_json::json!({
        "type": "doc",
        "content": [{"type":"paragraph","content":[{"type":"text","text":task}]}]
    })
    .to_string();
    let saved = state
        .save_draft(DraftInput {
            request_id: first_request_id.clone(),
            expected_revision: 0,
            document_json: document_json.clone(),
            body_markdown: task.to_owned(),
        })
        .await
        .expect("save live loop feedback");
    let revision = saved
        .attention_items
        .iter()
        .find_map(|item| match item {
            AttentionItem::Feedback {
                id, draft_revision, ..
            } if id == &first_request_id => Some(*draft_revision),
            _ => None,
        })
        .expect("saved live loop draft");
    state
        .submit_feedback(FeedbackDecisionInput {
            submission_id: format!("acceptance-feedback-{agent_id}-{run_id}"),
            request_id: first_request_id.clone(),
            expected_revision: revision,
            document_json,
            body_markdown: task.to_owned(),
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None,
        })
        .await
        .expect("submit live loop feedback");

    for attempt in 0..24 {
        let snapshot = state.read().await.expect("read resumed live Workbench");
        if let Some(item) = snapshot.attention_items.iter().find(|item| {
            item.session_id() == session_id
                && matches!(item, AttentionItem::Feedback { id, .. } if id != &first_request_id)
        }) {
            println!(
                "ACP_ACCEPTANCE_LOOP={}",
                serde_json::to_string(item).expect("serialize resumed attention item")
            );
            let received_feedback_body = matches!(
                item,
                AttentionItem::Feedback {
                    summary,
                    instructions,
                    ..
                } if summary.contains(CONTENT_MARKER) || instructions.contains(CONTENT_MARKER)
            );
            state.shutdown().await;
            assert!(
                received_feedback_body,
                "Agent reopened the Ramble Loop without receiving the submitted feedback body"
            );
            return;
        }
        println!("ACP_ACCEPTANCE_LOOP_WAIT={attempt}");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
    let failed = state
        .read()
        .await
        .expect("read failed live loop diagnostics");
    if let Some(timeline) = failed
        .timelines
        .iter()
        .find(|timeline| timeline.session_id == session_id)
    {
        println!(
            "ACP_ACCEPTANCE_LOOP_TIMELINE={}",
            serde_json::to_string(timeline).expect("serialize failed live timeline")
        );
    }
    let recovery = state
        .core
        .read_session_recovery(session_id.clone().into())
        .await
        .expect("read failed live recovery diagnostics");
    println!(
        "ACP_ACCEPTANCE_LOOP_RECOVERY={}",
        serde_json::to_string(&recovery.pending_agent_work)
            .expect("serialize failed live recovery")
    );
    state.shutdown().await;
    panic!("Agent consumed feedback but did not suspend the next work cycle with request_feedback");
}
