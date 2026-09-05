// Shared by the headless and desktop binary integration targets. Testing the
// actual executable catches accidental GUI setup and Windows pipe regressions.
use rambledesk_core::{
    AgentConfig, ManagedFeedbackProvider, NewManagedSession, SessionProtocol, SessionRepository,
};
use rambledesk_local_server::{
    AccessToken, LocalManagedFeedbackProvider, ServerConfig, start_server_with_managed,
};
use rambledesk_storage::SqliteFeedbackStore;
use serde_json::{Value, json};
use std::{collections::BTreeMap, process::Stdio, sync::Arc, time::Duration};
use tokio::{io::AsyncWriteExt, process::Command};

const ID: &str = "01992658-1250-7000-8000-000000000001";

fn command(directory: &std::path::Path, url: &str, token: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rambledesk"));
    command
        .arg("feedback")
        .current_dir(directory)
        .env("RAMBLEDESK_FEEDBACK_URL", url)
        .env("RAMBLEDESK_FEEDBACK_TOKEN", token)
        .env(
            "RAMBLEDESK_DATABASE_FILE",
            directory.join("must-not-open.sqlite3"),
        )
        .env(
            "RAMBLEDESK_LOCAL_SERVER_TOKEN_FILE",
            directory.join("must-not-create.token"),
        )
        .env(
            "RAMBLEDESK_LOCAL_SERVER_TOKEN",
            "unused-global-fixture-token",
        )
        .env("USERPROFILE", directory)
        .env("APPDATA", directory.join("roaming"))
        .env("LOCALAPPDATA", directory.join("local"))
        .env("XDG_CONFIG_HOME", directory.join("config"))
        .env("XDG_DATA_HOME", directory.join("data"))
        .env("RUST_LOG", "trace")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    command
}

async fn execute(mut command: Command, input: Option<&Value>, token: &str) -> (bool, Value) {
    let mut child = command.spawn().unwrap();
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&serde_json::to_vec(input).unwrap())
            .await
            .unwrap();
    } else {
        drop(child.stdin.take());
    }
    let output = tokio::time::timeout(Duration::from_secs(15), child.wait_with_output())
        .await
        .unwrap()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    for secret in [token, "unused-global-fixture-token"] {
        assert!(
            !stdout.contains(secret) && !stderr.contains(secret),
            "credential in output"
        );
    }
    assert!(stderr.is_empty(), "command must not initialize tracing");
    (
        output.status.success(),
        serde_json::from_str(&stdout).expect("one JSON result, no application logs"),
    )
}

#[tokio::test]
async fn binary_feedback_preserves_scope_unicode_packages_and_revocation_without_gui_setup() {
    let directory = tempfile::tempdir().unwrap();
    let store = SqliteFeedbackStore::connect(&directory.path().join("scope.sqlite3"))
        .await
        .unwrap();
    store
        .save_agent_config(AgentConfig {
            catalog_id: None,
            id: "fixture".into(),
            name: "Fixture".into(),
            host_id: "pi".into(),
            protocol: SessionProtocol::Acp,
            enabled: true,
            command: "unused".into(),
            args: vec![],
            env: BTreeMap::new(),
            created_at: "2026-09-05T00:00:00Z".into(),
            updated_at: "2026-09-05T00:00:00Z".into(),
        })
        .await
        .unwrap();
    let session = store
        .create_managed_session(NewManagedSession {
            session_id: "binary-feedback".into(),
            agent_config_id: "fixture".into(),
            cwd: directory.path().to_string_lossy().into_owned(),
            title: "Binary feedback".into(),
            created_at: "2026-09-05T00:00:00Z".into(),
        })
        .await
        .unwrap();
    let application = store.clone().into_application();
    let provider = Arc::new(LocalManagedFeedbackProvider::new(application.clone()));
    let server = start_server_with_managed(
        ServerConfig::new(AccessToken::generate()).with_port(0),
        application.clone(),
        provider.clone(),
    )
    .await
    .unwrap();
    let endpoint = provider.bind(&session).await.unwrap();
    let url = endpoint.url.replace("/mcp-managed", "/agent-feedback");
    let token = &endpoint.bearer_token;
    let payload = json!({"request_id":ID,"what_happened":"请检查中文与空格路径 🦊","actions":[{"id":"review","instruction":"请检查"}],"title":"请查看"});
    let mut request = command(directory.path(), &url, token);
    request.args(["request", "--input", "-"]);
    let (success, created) = execute(request, Some(&payload), token).await;
    assert!(success, "{created}");
    assert_eq!(created["managed_session_id"], session.session_id);
    assert_eq!(created["request_id"], ID);
    assert_eq!(created["status"], "waiting");
    assert!(created.get("poll_after_ms").is_none());
    let file = directory.path().join("反馈 request.json");
    let mut with_bom = vec![0xef, 0xbb, 0xbf];
    with_bom.extend(serde_json::to_vec(&payload).unwrap());
    std::fs::write(&file, with_bom).unwrap();
    let mut replay = command(directory.path(), &url, token);
    replay.args(["request", "--input"]).arg(&file);
    assert_eq!(execute(replay, None, token).await.1, created);
    application
        .cancel_feedback(rambledesk_core::CancelFeedbackInput {
            request_id: ID.into(),
            reason: "保持原任务".into(),
        })
        .await
        .unwrap();
    for operation in ["get", "recover"] {
        let mut read = command(directory.path(), &url, token);
        read.args([operation, "--request-id", ID]);
        let (success, result) = execute(read, None, token).await;
        assert!(success);
        assert_eq!(result["request_id"], ID);
        assert_eq!(result["status"], "cancelled");
        assert!(result.get("feedback_package").is_some());
    }
    provider.revoke(&session.session_id).await.unwrap();
    let mut revoked = command(directory.path(), &url, token);
    revoked.args(["get", "--request-id", ID]);
    let (success, result) = execute(revoked, None, token).await;
    assert!(!success);
    assert_eq!(result["code"], "revoked_capability");
    assert_eq!(result["request_id"], ID);
    for path in [
        "must-not-open.sqlite3",
        "must-not-create.token",
        "local",
        "roaming",
    ] {
        assert!(
            !directory.path().join(path).exists(),
            "command entered desktop initialization"
        );
    }
    server.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn binary_feedback_missing_capability_never_uses_external_credentials() {
    let directory = tempfile::tempdir().unwrap();
    let mut missing = command(directory.path(), "http://127.0.0.1:1/agent-feedback", "ab");
    missing
        .env_remove("RAMBLEDESK_FEEDBACK_TOKEN")
        .args(["recover"]);
    let (success, result) = execute(missing, None, "never-printed-secret").await;
    assert!(!success);
    assert_eq!(result["code"], "missing_capability");
    assert!(!directory.path().join("must-not-create.token").exists());
}
