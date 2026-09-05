mod support;

use rambledesk_acp::AcpSessionDriver;
use rambledesk_core::*;
use rambledesk_storage::SqliteFeedbackStore;
use std::{sync::Arc, time::Duration};
use support::{create, id, setup, wait_for};

async fn idle(app: &SessionApplication, session: ManagedSessionInput) -> ManagedSessionSnapshot {
    wait_for(app, &session, |snapshot| {
        snapshot.runtime.activity == SessionActivityState::Idle
    })
    .await
}

async fn permissions(
    app: &SessionApplication,
    session: ManagedSessionInput,
) -> ManagedSessionSnapshot {
    wait_for(app, &session, |snapshot| !snapshot.permissions.is_empty()).await
}

#[tokio::test]
async fn permission_queue_is_scoped_validated_and_consumed_once() {
    let (dir, store, app, config) = setup("agent", "load").await;
    let first = create(&app, &dir, &config, "One").await;
    let other = create(&app, &dir, &config, "Two").await;
    app.send_prompt(SendManagedPromptInput {
        session_id: first.session.session_id.clone(),
        text: "permission_pair".into(),
    })
    .await
    .unwrap();
    // Both independent callbacks must arrive before inspecting the full queue.
    let first = wait_for(&app, &id(&first), |snapshot| {
        snapshot.permissions.len() == 2
    })
    .await;
    let details = first
        .permissions
        .iter()
        .find_map(|permission| permission.details.as_deref())
        .unwrap();
    assert!(details.contains("cargo check"));
    assert!(details.contains("C:/fixture-project/Cargo.toml:4"));
    assert_eq!(
        first
            .permissions
            .iter()
            .filter(|permission| permission.details.is_none())
            .count(),
        1
    );
    assert_eq!(
        first.runtime.activity,
        SessionActivityState::WaitingPermission
    );
    let answer = RespondManagedPermissionInput {
        session_id: first.session.session_id.clone(),
        request_id: first.permissions[0].request_id.clone(),
        option_id: Some("allow".into()),
    };
    assert!(
        app.respond_permission(RespondManagedPermissionInput {
            session_id: other.session.session_id.clone(),
            ..answer.clone()
        })
        .await
        .is_err()
    );
    assert!(
        app.respond_permission(RespondManagedPermissionInput {
            option_id: Some("invented".into()),
            ..answer.clone()
        })
        .await
        .is_err()
    );
    assert_eq!(
        app.get_session(id(&first)).await.unwrap().permissions.len(),
        2
    );
    let remaining = app.respond_permission(answer.clone()).await.unwrap();
    assert_eq!(remaining.permissions.len(), 1);
    assert!(app.respond_permission(answer).await.is_err());
    app.respond_permission(RespondManagedPermissionInput {
        session_id: first.session.session_id.clone(),
        request_id: remaining.permissions[0].request_id.clone(),
        option_id: None,
    })
    .await
    .unwrap();
    assert!(idle(&app, id(&first)).await.permissions.is_empty());
    assert_eq!(
        app.get_session(id(&other)).await.unwrap().runtime.activity,
        SessionActivityState::Idle
    );
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn cancellation_drains_permissions_and_does_not_cancel_a_later_turn() {
    let (dir, store, app, config) = setup("agent", "load").await;
    let first = create(&app, &dir, &config, "One").await;
    app.send_prompt(SendManagedPromptInput {
        session_id: first.session.session_id.clone(),
        text: "permission_pair".into(),
    })
    .await
    .unwrap();
    let first = permissions(&app, id(&first)).await;
    app.cancel_prompt(id(&first)).await.unwrap();
    let done = idle(&app, id(&first)).await;
    assert!(done.permissions.is_empty());
    app.send_prompt(SendManagedPromptInput {
        session_id: first.session.session_id.clone(),
        text: "wait".into(),
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(5300)).await;
    let running = app.get_session(id(&first)).await.unwrap();
    assert_eq!(
        running.runtime.connection,
        SessionConnectionState::Connected
    );
    assert_eq!(running.runtime.activity, SessionActivityState::Running);
    app.cancel_prompt(id(&first)).await.unwrap();
    idle(&app, id(&first)).await;
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn uncooperative_cancel_stops_only_the_owned_instance() {
    let (dir, store, app, config) = setup("agent", "load").await;
    let mut saved = store.get_agent_config(&config).await.unwrap();
    saved.args[1] = "ignore_cancel".into();
    store.save_agent_config(saved).await.unwrap();
    let first = create(&app, &dir, &config, "One").await;
    let other = create(&app, &dir, &config, "Two").await;
    app.send_prompt(SendManagedPromptInput {
        session_id: first.session.session_id.clone(),
        text: "wait".into(),
    })
    .await
    .unwrap();
    app.cancel_prompt(id(&first)).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let state = app.get_session(id(&first)).await.unwrap();
            if state.runtime.connection == SessionConnectionState::Stopped {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        app.get_session(id(&other))
            .await
            .unwrap()
            .runtime
            .connection,
        SessionConnectionState::Connected
    );
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn two_instances_stream_to_their_own_durable_activity_and_resume_without_replay() {
    let (dir, store, app, config) = setup("agent", "load").await;
    let first = create(&app, &dir, &config, "One").await;
    let second = create(&app, &dir, &config, "Two").await;
    let (a, b) = tokio::join!(
        app.send_prompt(SendManagedPromptInput {
            session_id: first.session.session_id.clone(),
            text: "ALPHA".into()
        }),
        app.send_prompt(SendManagedPromptInput {
            session_id: second.session.session_id.clone(),
            text: "BETA".into()
        })
    );
    a.unwrap();
    b.unwrap();
    let first = idle(&app, id(&first)).await;
    let second = idle(&app, id(&second)).await;
    for (snapshot, word, other) in [(&first, "ALPHA", "BETA"), (&second, "BETA", "ALPHA")] {
        let messages = snapshot
            .activities
            .iter()
            .filter(|row| row.kind == SessionActivityKind::AgentMessage)
            .collect::<Vec<_>>();
        assert_eq!(
            messages.len(),
            1,
            "stream chunks should form one durable message"
        );
        assert_eq!(messages[0].text, format!("fixture reply: {word}"));
        assert!(
            snapshot
                .activities
                .iter()
                .all(|row| row.session_id == snapshot.session.session_id
                    && !row.text.contains(other))
        );
    }
    app.shutdown().await.unwrap();
    store.close().await;
    let store = Arc::new(
        SqliteFeedbackStore::connect(&dir.path().join("db.sqlite"))
            .await
            .unwrap(),
    );
    let app = SessionApplication::new(store.clone(), store.clone(), Arc::new(AcpSessionDriver));
    let before = app.get_session(id(&first)).await.unwrap();
    assert_eq!(before.runtime.connection, SessionConnectionState::Stopped);
    assert_eq!(before.activities, first.activities);
    let resumed = app.start_session(id(&first)).await.unwrap();
    assert_eq!(resumed.session.management, first.session.management);
    assert_eq!(
        resumed.activities, first.activities,
        "startup replay must not duplicate the timeline"
    );
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn busy_session_rejects_duplicate_input_and_stopping_it_keeps_other_instance_connected() {
    let (dir, store, app, config) = setup("agent", "load").await;
    let first = create(&app, &dir, &config, "One").await;
    let second = create(&app, &dir, &config, "Two").await;
    app.send_prompt(SendManagedPromptInput {
        session_id: first.session.session_id.clone(),
        text: "wait".into(),
    })
    .await
    .unwrap();
    assert!(matches!(
        app.send_prompt(SendManagedPromptInput {
            session_id: first.session.session_id.clone(),
            text: "duplicate".into()
        })
        .await,
        Err(SessionError::Busy)
    ));
    app.stop_session(id(&first)).await.unwrap();
    assert_eq!(
        app.get_session(id(&second))
            .await
            .unwrap()
            .runtime
            .connection,
        SessionConnectionState::Connected
    );
    assert!(
        app.get_session(id(&first))
            .await
            .unwrap()
            .activities
            .iter()
            .all(|row| row.text != "duplicate")
    );
    app.shutdown().await.unwrap();
    store.close().await;
}
