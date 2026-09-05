use rambledesk_acp::AcpSessionDriver;
use rambledesk_core::*;
use rambledesk_storage::SqliteFeedbackStore;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jP1sAAAAASUVORK5CYII=";
fn image() -> SessionPromptContent {
    SessionPromptContent::Image {
        mime_type: "image/png".into(),
        data: PNG.into(),
    }
}
async fn setup(
    full: bool,
) -> (
    tempfile::TempDir,
    Arc<SqliteFeedbackStore>,
    SessionApplication,
    ManagedSessionInput,
) {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteFeedbackStore::connect(&dir.path().join("db.sqlite"))
            .await
            .unwrap(),
    );
    let app = SessionApplication::new(store.clone(), store.clone(), Arc::new(AcpSessionDriver));
    let config = app
        .save_agent_config(SaveAgentConfigInput {
            catalog_id: None,
            id: None,
            name: "Typed fixture".into(),
            host_id: "fixture".into(),
            protocol: SessionProtocol::Acp,
            enabled: true,
            command: "node".into(),
            args: vec![
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/prompt_content.mjs")
                    .to_string_lossy()
                    .into(),
                if full { "full" } else { "baseline" }.into(),
            ],
            env: BTreeMap::new(),
        })
        .await
        .unwrap();
    let session = app
        .create_session(CreateManagedSessionInput {
            agent_config_id: config.id,
            cwd: dir.path().to_string_lossy().into(),
            title: "Typed".into(),
        })
        .await
        .unwrap();
    assert_eq!(
        session.runtime.connection,
        SessionConnectionState::Connected
    );
    assert_eq!(session.runtime.capabilities.prompt.image, full);
    assert_eq!(session.runtime.capabilities.prompt.embedded_context, full);
    assert!(session.runtime.capabilities.prompt.resource_links);
    (
        dir,
        store,
        app,
        ManagedSessionInput {
            session_id: session.session.session_id,
        },
    )
}
fn input(
    id: &ManagedSessionInput,
    text: &str,
    content: Vec<SessionPromptContent>,
) -> SendManagedPromptContentInput {
    SendManagedPromptContentInput {
        session_id: id.session_id.clone(),
        text: text.into(),
        content,
    }
}
async fn idle(app: &SessionApplication, id: &ManagedSessionInput) -> ManagedSessionSnapshot {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let snapshot = app.get_session(id.clone()).await.unwrap();
            if snapshot.runtime.activity == SessionActivityState::Idle {
                return snapshot;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn text_image_and_resources_reach_agent_in_order_and_reload_as_one_user_message() {
    let (dir, store, app, id) = setup(true).await;
    let blocks = vec![
        image(),
        SessionPromptContent::ResourceLink {
            uri: "file:///does-not-exist.txt".into(),
            name: "Reference".into(),
            mime_type: Some("text/plain".into()),
        },
        SessionPromptContent::Resource {
            uri: "file:///embedded.md".into(),
            mime_type: Some("text/markdown".into()),
            text: "Embedded context".into(),
        },
    ];
    app.send_prompt_content(input(&id, "Inspect", blocks))
        .await
        .unwrap();
    let snapshot = idle(&app, &id).await;
    let users = snapshot
        .activities
        .iter()
        .filter(|row| row.kind == SessionActivityKind::UserMessage)
        .collect::<Vec<_>>();
    assert_eq!(users.len(), 1);
    let Some(SessionActivityContent::Message { blocks, truncated }) = &users[0].content else {
        panic!("typed user history")
    };
    assert!(!truncated);
    assert_eq!(blocks.len(), 4);
    assert!(
        matches!(&blocks[1], SessionContentBlock::Image { data: Some(data), .. } if data == PNG)
    );
    let response = snapshot
        .activities
        .iter()
        .find(|row| row.kind == SessionActivityKind::AgentMessage)
        .unwrap();
    let seen: serde_json::Value = serde_json::from_str(&response.text).unwrap();
    assert_eq!(seen[0]["text"], "Inspect");
    assert_eq!(seen[1]["exactPng"], true);
    assert_eq!(seen[1]["mime"], "image/png");
    assert_eq!(seen[2]["uri"], "file:///does-not-exist.txt");
    assert_eq!(seen[3]["text"], "Embedded context");
    app.shutdown().await.unwrap();
    store.close().await;
    let reopened = SqliteFeedbackStore::connect(&dir.path().join("db.sqlite"))
        .await
        .unwrap();
    assert_eq!(
        reopened
            .list_session_activity(&id.session_id, None, 100)
            .await
            .unwrap(),
        snapshot.activities
    );
    reopened.close().await;
}

#[tokio::test]
async fn unsupported_and_invalid_content_is_rejected_before_a_turn_or_history_row() {
    let (_dir, store, app, id) = setup(false).await;
    for blocks in [
        vec![image()],
        vec![SessionPromptContent::Resource {
            uri: "file:///context".into(),
            mime_type: None,
            text: "Context".into(),
        }],
        vec![SessionPromptContent::ResourceLink {
            uri: "javascript://bad".into(),
            name: "Bad".into(),
            mime_type: None,
        }],
    ] {
        assert!(matches!(
            app.send_prompt_content(input(&id, "Inspect", blocks)).await,
            Err(SessionError::InvalidInput)
        ));
    }
    assert!(
        app.get_session(id.clone())
            .await
            .unwrap()
            .activities
            .is_empty()
    );
    app.send_prompt_content(input(
        &id,
        "Read reference",
        vec![SessionPromptContent::ResourceLink {
            uri: "https://example.test/context".into(),
            name: "Reference".into(),
            mime_type: None,
        }],
    ))
    .await
    .unwrap();
    assert!(
        idle(&app, &id)
            .await
            .activities
            .iter()
            .any(|row| row.kind == SessionActivityKind::AgentMessage)
    );
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn typed_cancel_does_not_duplicate_user_content_and_plain_text_still_works() {
    let (_dir, store, app, id) = setup(true).await;
    app.send_prompt_content(input(&id, "wait", vec![image()]))
        .await
        .unwrap();
    app.cancel_prompt(id.clone()).await.unwrap();
    let cancelled = idle(&app, &id).await;
    assert_eq!(
        cancelled
            .activities
            .iter()
            .filter(|row| row.kind == SessionActivityKind::UserMessage)
            .count(),
        1
    );
    app.send_prompt(SendManagedPromptInput {
        session_id: id.session_id.clone(),
        text: "Legacy".into(),
    })
    .await
    .unwrap();
    let final_state = idle(&app, &id).await;
    let users = final_state
        .activities
        .iter()
        .filter(|row| row.kind == SessionActivityKind::UserMessage)
        .collect::<Vec<_>>();
    assert_eq!(users.len(), 2);
    assert!(users[0].content.is_some());
    assert_eq!(users[1].content, None);
    assert_eq!(users[1].text, "Legacy");
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn full_image_crosses_protocol_while_history_uses_bounded_preview() {
    let (_dir, store, app, id) = setup(true).await;
    // Valid base64 with a PNG signature, larger than the display-preview cap.
    let data = format!("iVBORw0KGgoA{}", "AAAA".repeat(120_000));
    let encoded_len = data.len();
    app.send_prompt_content(input(
        &id,
        "Inspect full image",
        vec![SessionPromptContent::Image {
            mime_type: "image/png".into(),
            data,
        }],
    ))
    .await
    .unwrap();
    let snapshot = idle(&app, &id).await;
    let user = snapshot
        .activities
        .iter()
        .find(|row| row.kind == SessionActivityKind::UserMessage)
        .unwrap();
    let Some(SessionActivityContent::Message { blocks, truncated }) = &user.content else {
        panic!("typed history")
    };
    assert!(*truncated);
    assert!(matches!(
        blocks[1],
        SessionContentBlock::Image { data: None, .. }
    ));
    assert!(
        matches!(&blocks[0], SessionContentBlock::Text { text } if text == "Inspect full image")
    );
    let response = snapshot
        .activities
        .iter()
        .find(|row| row.kind == SessionActivityKind::AgentMessage)
        .unwrap();
    let seen: serde_json::Value = serde_json::from_str(&response.text).unwrap();
    assert_eq!(seen[1]["bytes"], encoded_len);
    app.shutdown().await.unwrap();
    store.close().await;
}
