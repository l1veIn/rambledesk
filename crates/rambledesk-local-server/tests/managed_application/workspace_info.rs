use super::*;

#[tokio::test]
async fn workspace_query_uses_saved_directory_and_observes_git_changes_without_connecting()
-> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let config = fixture.facade.save_agent_config(config_input()).await?;
    let cwd = fixture.directory.path().join("project");
    std::fs::create_dir_all(cwd.join(".git"))?;
    std::fs::write(cwd.join(".git/HEAD"), "ref: refs/heads/codex/agent-ui\n")?;
    let session = fixture
        .store
        .create_managed_session(NewManagedSession {
            session_id: uuid::Uuid::now_v7().to_string(),
            agent_config_id: config.id,
            cwd: cwd.to_string_lossy().into_owned(),
            title: "Workspace metadata".into(),
            created_at: "2026-09-05T12:00:00Z".into(),
        })
        .await?;
    // No runtime-generation mutation fence is needed, and extra client input
    // cannot redirect this query to another local path.
    let response = fixture
        .request(
            "getManagedWorkspaceInfo",
            json!({
                "session_id": session.session_id,
                "cwd": fixture.directory.path(),
            }),
        )
        .send()
        .await?
        .error_for_status()?;
    assert!(response.headers().contains_key(REVISION_HEADER));
    assert_eq!(
        response.json::<Value>().await?,
        json!({
            "cwd": cwd,
            "branch": "codex/agent-ui",
        })
    );
    std::fs::write(cwd.join(".git/HEAD"), "ref: refs/heads/main\n")?;
    assert_eq!(
        fixture
            .call(
                "getManagedWorkspaceInfo",
                json!({"session_id": session.session_id})
            )
            .await?["branch"],
        "main"
    );
    std::fs::remove_file(cwd.join(".git/HEAD"))?;
    assert_eq!(
        fixture
            .call(
                "getManagedWorkspaceInfo",
                json!({"session_id": session.session_id})
            )
            .await?["branch"],
        Value::Null
    );
    let status = fixture
        .facade
        .get_managed_feedback_status(ManagedSessionInput {
            session_id: session.session_id,
        })
        .await?;
    assert_eq!(status.connection, SessionConnectionState::Stopped);
    assert_eq!(fixture.driver.checks.load(Ordering::SeqCst), 0);
    fixture.shutdown().await
}

#[tokio::test]
async fn workspace_query_rejects_unknown_session() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let response = fixture
        .request(
            "getManagedWorkspaceInfo",
            json!({
                "session_id": uuid::Uuid::now_v7().to_string(),
            }),
        )
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
    assert_eq!(
        response.json::<Value>().await?["code"],
        "MANAGED_SESSION_NOT_FOUND"
    );
    fixture.shutdown().await
}

#[tokio::test]
async fn workspace_query_does_not_accept_external_session_source_hints() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    fixture
        .feedback
        .request_feedback(RequestFeedbackInput {
            request_id: None,
            host_id: Some("generic".into()),
            host_session_id: "external-conversation".into(),
            title: Some("External feedback".into()),
            what_happened: "Review external work".into(),
            actions: vec![ActionInput {
                id: "review".into(),
                instruction: "Review".into(),
            }],
            context_refs: vec![],
            attachments: vec![],
            source_hint: Some(fixture.directory.path().to_string_lossy().into_owned()),
            allow_finish: false,
            final_summary: None,
        })
        .await?;
    let external = fixture.facade.list_host_sessions().await?.pop().unwrap();
    let response = fixture
        .request(
            "getManagedWorkspaceInfo",
            json!({
                "session_id": external.session_id,
            }),
        )
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(
        response.json::<Value>().await?["code"],
        "SESSION_NOT_MANAGED"
    );
    fixture.shutdown().await
}
