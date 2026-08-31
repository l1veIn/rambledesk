use std::sync::Arc;

use rambledesk_acp_client::{
    AcpClient, AcpClientConfig, AgentLaunchConfig, LaunchConfigKind, LaunchConfigSelection,
    LaunchProfile, PermissionAnswer, RecoveryMethod, SessionScope,
};
use rambledesk_core::kernel::{
    AccessMode, ArtifactInput, Core, LaunchConfiguration, LaunchSubmission, RambleContent,
    SubmissionId,
};
use rambledesk_storage::v3::{SqliteV3Store, artifact::LocalArtifactStore};
use tempfile::TempDir;

/// Opt-in probe for the pinned first-party Adapter. It may download the npm
/// package and requires the user's normal Codex authentication, so CI and the
/// default test suite deliberately skip it.
#[tokio::test]
#[ignore = "requires npx, network access, and a locally authenticated Codex installation"]
async fn pinned_codex_acp_negotiates_the_v1_contract() {
    let temp = TempDir::new().expect("temporary smoke workspace");
    let store = Arc::new(
        SqliteV3Store::connect(&temp.path().join("v3.sqlite3"))
            .await
            .expect("v3 store"),
    );
    let artifacts = Arc::new(
        LocalArtifactStore::open(temp.path().join("library"))
            .await
            .expect("artifact store"),
    );
    let client = AcpClient::new(
        Arc::new(Core::new(store.clone(), artifacts)),
        AcpClientConfig {
            operation_timeout: std::time::Duration::from_secs(60),
            ..AcpClientConfig::default()
        },
    )
    .expect("ACP Client");

    let report = client
        .preflight(LaunchProfile::codex_npx().profile_ref, temp.path())
        .await
        .expect("pinned Codex ACP preflight");
    assert_eq!(report.capabilities.protocol_version, 1);
    assert!(report.capabilities.resume_session);
    assert!(report.capabilities.mcp_http);
    println!(
        "{}",
        serde_json::to_string_pretty(&report.config_options).expect("display config options")
    );
    for (id, category) in [
        ("model", "model"),
        ("reasoning_effort", "thought_level"),
        ("mode", "mode"),
    ] {
        let option = report
            .config_options
            .iter()
            .find(|option| option.id == id)
            .unwrap_or_else(|| panic!("Codex omitted {id} config option"));
        assert_eq!(option.category.as_deref(), Some(category));
        let LaunchConfigKind::Select { options, .. } = &option.kind else {
            panic!("Codex {id} option was not selectable")
        };
        assert!(!options.is_empty());
    }
    let mode = report
        .config_options
        .iter()
        .find(|option| option.id == "mode")
        .unwrap();
    let LaunchConfigKind::Select { options, .. } = &mode.kind else {
        panic!("Codex mode option was not selectable")
    };
    let mode_values = options
        .iter()
        .map(|option| option.value.as_str())
        .collect::<Vec<_>>();
    assert!(mode_values.contains(&"read-only"));
    assert!(mode_values.contains(&"agent"));
    assert!(mode_values.contains(&"agent-full-access"));

    store.close().await;
}

#[tokio::test]
#[ignore = "uses a real Codex turn to verify Permission pass-through and session resume"]
async fn pinned_codex_acp_passes_through_permission_and_resumes_after_close() {
    let temp = TempDir::new().expect("temporary smoke workspace");
    let store = Arc::new(
        SqliteV3Store::connect(&temp.path().join("v3.sqlite3"))
            .await
            .expect("v3 store"),
    );
    let artifacts = Arc::new(
        LocalArtifactStore::open(temp.path().join("library"))
            .await
            .expect("artifact store"),
    );
    let core = Arc::new(Core::new(store.clone(), artifacts));
    let config = AcpClientConfig {
        operation_timeout: std::time::Duration::from_secs(60),
        ..AcpClientConfig::default()
    };
    let client = AcpClient::new(core.clone(), config.clone()).expect("ACP Client");
    let preflight = client
        .preflight(LaunchProfile::codex_npx().profile_ref, temp.path())
        .await
        .expect("preflight");
    let config_values = preflight
        .config_options
        .iter()
        .filter_map(|option| match &option.kind {
            LaunchConfigKind::Select { current_value, .. } => Some(LaunchConfigSelection {
                id: option.id.clone(),
                value: serde_json::json!(current_value),
            }),
            LaunchConfigKind::Boolean { current_value } => Some(LaunchConfigSelection {
                id: option.id.clone(),
                value: serde_json::json!(current_value),
            }),
            LaunchConfigKind::Unsupported { .. } => None,
        })
        .collect();
    let launch = core
        .launch(LaunchSubmission {
            submission_id: SubmissionId::new("codex-permission-smoke"),
            submission_digest_assertion: None,
            title: "Codex Permission Smoke".to_string(),
            launch_configuration: LaunchConfiguration {
                agent_profile_id: "codex".to_string(),
                launch_profile_id: "codex-acp-npx".to_string(),
                workspace_reference: temp.path().to_string_lossy().to_string(),
                model: None,
                reasoning_effort: None,
                access_mode: AccessMode::WorkspaceWrite,
                agent_config_json: serde_json::to_string(&AgentLaunchConfig {
                    version: 1,
                    schema_digest: preflight.schema_digest,
                    values: config_values,
                })
                .expect("serialize Agent config"),
            },
            ramble: RambleContent {
                document_json: "{}".to_string(),
                body_markdown: "Run exactly `curl -I https://example.com` to verify that network access asks me for permission. Do not use another method. After I deny the request, end the turn.".to_string(),
                artifacts: Vec::<ArtifactInput>::new(),
            },
        })
        .await
        .expect("launch fact");
    client
        .reconcile(SessionScope {
            session_id: launch.session_id.clone(),
        })
        .await
        .expect("launch real Agent Run");
    let permission = tokio::time::timeout(std::time::Duration::from_secs(90), async {
        loop {
            let snapshot = client
                .reconcile(SessionScope {
                    session_id: launch.session_id.clone(),
                })
                .await
                .expect("live snapshot");
            if let Some(permission) = snapshot.permissions.into_iter().next() {
                break permission;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("Codex Permission Request");
    println!(
        "Permission meta: {}",
        serde_json::to_string_pretty(&permission.request_meta).unwrap()
    );
    let reject = permission
        .options
        .iter()
        .find(|option| option.kind.contains("reject"))
        .expect("Agent-provided reject option");
    client
        .answer_permission(PermissionAnswer {
            session_id: launch.session_id.clone(),
            live_request_id: permission.live_request_id,
            option_id: reject.option_id.clone(),
        })
        .await
        .expect("deny Permission Request");
    client.shutdown().await.expect("close first Agent Run");

    let resumed = AcpClient::new(core, config).expect("resuming ACP Client");
    let snapshot = resumed
        .reconcile(SessionScope {
            session_id: launch.session_id,
        })
        .await
        .expect("resume closed Codex Session");
    assert_eq!(snapshot.recovery_method, RecoveryMethod::Resume);
    resumed.shutdown().await.expect("close resumed Agent Run");
    store.close().await;
}
