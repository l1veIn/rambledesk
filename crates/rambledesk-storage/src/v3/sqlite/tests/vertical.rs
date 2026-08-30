use std::sync::Arc;

use rambledesk_core::kernel::ports::ArtifactStore;

use super::*;
use crate::v3::artifact::LocalArtifactStore;

#[tokio::test]
async fn core_sqlite_and_local_artifacts_form_a_restart_safe_launch_slice() {
    let temp = TempDir::new().expect("tempdir");
    let database = temp.path().join("vertical.sqlite3");
    let store = SqliteV3Store::connect(&database).await.expect("v3 store");
    let artifacts = LocalArtifactStore::open(temp.path())
        .await
        .expect("artifact store");
    let core = Core::new(Arc::new(store.clone()), Arc::new(artifacts.clone()));
    let input = LaunchSubmission {
        submission_id: SubmissionId::new("vertical-launch"),
        submission_digest_assertion: None,
        title: "Vertical launch".to_owned(),
        launch_configuration: LaunchConfiguration {
            agent_profile_id: "codex".to_owned(),
            launch_profile_id: "local".to_owned(),
            workspace_reference: "/workspace".to_owned(),
            model: Some("gpt-5".to_owned()),
            reasoning_effort: Some("high".to_owned()),
            access_mode: AccessMode::WorkspaceWrite,
            agent_config_json: "opaque-agent-config".to_owned(),
        },
        ramble: RambleContent {
            document_json: "opaque-document".to_owned(),
            body_markdown: "Launch from the real Core.".to_owned(),
            artifacts: vec![ArtifactInput {
                display_name: "evidence.txt".to_owned(),
                media_type: "text/plain".to_owned(),
                contents: b"portable evidence".to_vec(),
            }],
        },
    };
    let first = core.launch(input.clone()).await.expect("first launch");
    assert_eq!(core.launch(input).await.expect("idempotent launch"), first);

    let manifest: String =
        sqlx::query_scalar("SELECT manifest_json FROM packages_v3 WHERE package_id = ?")
            .bind(first.package_id.as_str())
            .fetch_one(&store.pool)
            .await
            .expect("manifest");
    assert!(!manifest.contains("storage_key"));
    assert!(!manifest.contains(&temp.path().display().to_string()));
    let blob: (String, String) = sqlx::query_as(
        "SELECT storage_key, sha256 FROM package_artifacts_v3
         WHERE package_id = ? AND display_name = 'evidence.txt'",
    )
    .bind(first.package_id.as_str())
    .fetch_one(&store.pool)
    .await
    .expect("package artifact");
    assert_eq!(
        artifacts
            .open_verified(&blob.0, &blob.1)
            .await
            .expect("verified artifact"),
        b"portable evidence"
    );
    drop(core);
    store.close().await;

    let reopened = SqliteV3Store::connect(&database)
        .await
        .expect("reopen store");
    let recovery = match reopened
        .query(FactQuery::SessionRecovery(first.session_id.clone()))
        .await
        .expect("recover launch")
    {
        FactQueryOutcome::SessionRecovery(value) => value,
        _ => panic!("wrong recovery outcome"),
    };
    assert_eq!(
        recovery
            .launch_submission
            .expect("launch submission")
            .submission_digest,
        first.submission_digest
    );
}
