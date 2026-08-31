use rambledesk_core::kernel::AccessMode;

use super::super::AcpWorkbenchState;
use super::super::model::{AttentionItem, LaunchDraftInput};

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

    let probe = LaunchDraftInput {
        submission_id: format!("acceptance-{agent_id}-{run_id}"),
        workspace: workspace.to_string_lossy().into_owned(),
        agent_id: agent_id.clone(),
        model: String::new(),
        reasoning_effort: String::new(),
        access_mode: AccessMode::WorkspaceWrite,
        document_json: r#"{"type":"doc"}"#.to_owned(),
        body_markdown: "Call `request_feedback` now to ask the human what they want to work on. Do not guess their intent or start work before their feedback is submitted.".to_owned(),
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
    if mode != "launch" {
        state.shutdown().await;
        return;
    }

    let preferred_access = std::env::var("RAMBLEDESK_ACP_TEST_ACCESS").ok();
    let access_mode = preferred_access
        .as_deref()
        .and_then(|value| match value {
            "read_only" => Some(AccessMode::ReadOnly),
            "workspace_write" => Some(AccessMode::WorkspaceWrite),
            "yolo" => Some(AccessMode::Yolo),
            _ => None,
        })
        .filter(|value| preflight.access_modes.contains(value))
        .or_else(|| preflight.access_modes.first().copied())
        .expect("the Agent connected but has no verified RambleDesk access-mode mapping");
    let input = LaunchDraftInput {
        model: std::env::var("RAMBLEDESK_ACP_TEST_MODEL")
            .ok()
            .filter(|value| preflight.models.contains(value))
            .or_else(|| preflight.models.first().cloned())
            .unwrap_or_default(),
        reasoning_effort: std::env::var("RAMBLEDESK_ACP_TEST_REASONING")
            .ok()
            .filter(|value| preflight.reasoning_efforts.contains(value))
            .or_else(|| preflight.reasoning_efforts.first().cloned())
            .unwrap_or_default(),
        access_mode,
        ..probe
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
        .find(|session| session.agent_id == agent_id)
        .expect("launched session")
        .session_id
        .clone();
    for attempt in 0..18 {
        let snapshot = state.read().await.expect("read live Workbench");
        if let Some(item) = snapshot.attention_items.iter().find(|item| {
            item.session_id() == session_id && matches!(item, AttentionItem::Feedback { .. })
        }) {
            println!(
                "ACP_ACCEPTANCE_RAMBLE={}",
                serde_json::to_string(item).expect("serialize attention item")
            );
            state.shutdown().await;
            return;
        }
        println!("ACP_ACCEPTANCE_WAIT={attempt}");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
    state.shutdown().await;
    panic!("Agent completed launch without producing a RambleDesk attention request");
}
