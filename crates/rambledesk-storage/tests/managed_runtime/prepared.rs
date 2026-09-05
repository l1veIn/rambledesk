use super::*;

fn prepare(dir: &tempfile::TempDir, config: &str) -> PrepareManagedSessionInput {
    PrepareManagedSessionInput {
        agent_config_id: config.into(),
        cwd: dir.path().to_string_lossy().into_owned(),
    }
}

fn prompt(snapshot: &ManagedSessionSnapshot) -> SendManagedPromptInput {
    SendManagedPromptInput {
        session_id: snapshot.session.session_id.clone(),
        text: "  Build the report\nusing this workspace  ".into(),
    }
}

#[tokio::test]
async fn prepared_connection_is_hidden_until_first_prompt_and_reuses_remote_context() {
    let (dir, store, driver, app, config) = setup().await;
    let ready = app.prepare_session(prepare(&dir, &config)).await.unwrap();
    assert!(ready.session.is_prepared());
    assert_eq!(ready.runtime.connection, SessionConnectionState::Connected);
    assert_eq!(
        ready
            .runtime
            .configuration
            .models
            .as_ref()
            .unwrap()
            .current_model_id,
        "fixture-model"
    );
    assert!(ready.activities.is_empty());
    assert!(store.list_managed_sessions().await.unwrap().is_empty());
    assert!(
        store
            .list_host_sessions(HostSessionQuery {
                archived: false,
                search: None
            })
            .await
            .unwrap()
            .is_empty()
    );
    let scope = ManagedFeedbackScope::from_session(&ready.session).unwrap();
    let feedback = store.as_ref().clone().into_application();
    let rejected = feedback
        .request_managed_feedback(
            &scope,
            RequestFeedbackInput {
                request_id: None,
                host_id: None,
                host_session_id: String::new(),
                title: None,
                what_happened: "No user prompt yet".into(),
                actions: vec![ActionInput {
                    id: "review".into(),
                    instruction: "Review this".into(),
                }],
                context_refs: vec![],
                attachments: vec![],
                source_hint: None,
                allow_finish: false,
                final_summary: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(rejected.code(), "REQUEST_NOT_FOUND");
    let sent = app.send_prompt(prompt(&ready)).await.unwrap();
    assert!(!sent.session.is_prepared());
    assert_eq!(sent.session.lifecycle, Some(SessionLifecycle::Active));
    assert_eq!(sent.session.session_id, ready.session.session_id);
    assert_eq!(sent.session.management, ready.session.management);
    assert_eq!(sent.session.title, "Build the report using this workspace");
    assert_eq!(driver.starts.lock().unwrap().len(), 1);
    assert_eq!(store.list_managed_sessions().await.unwrap().len(), 1);
    assert_eq!(
        store
            .list_host_sessions(HostSessionQuery {
                archived: false,
                search: None
            })
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        sent.activities
            .iter()
            .filter(|row| row.kind == SessionActivityKind::UserMessage)
            .count(),
        1
    );
    assert!(matches!(
        app.discard_prepared_session(target(&sent)).await,
        Err(SessionError::Repository(SessionRepositoryError::Conflict))
    ));
    assert!(
        !driver.connections.lock().unwrap()[0]
            .closed
            .load(Ordering::SeqCst)
    );
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn failed_preparation_and_unsent_prompt_can_retry_the_same_draft() {
    let (dir, store, driver, app, config) = setup().await;
    driver.fail.store(true, Ordering::SeqCst);
    let failed = app.prepare_session(prepare(&dir, &config)).await.unwrap();
    assert!(failed.session.is_prepared());
    assert_eq!(failed.runtime.connection, SessionConnectionState::Failed);
    assert!(matches!(
        app.send_prompt(prompt(&failed)).await,
        Err(SessionError::NotConnected)
    ));
    assert!(
        store
            .get_session(&failed.session.session_id)
            .await
            .unwrap()
            .is_prepared()
    );
    assert!(store.list_managed_sessions().await.unwrap().is_empty());
    driver.fail.store(false, Ordering::SeqCst);
    let ready = app.start_session(target(&failed)).await.unwrap();
    assert_eq!(ready.session.session_id, failed.session.session_id);
    let sent = app.send_prompt(prompt(&ready)).await.unwrap();
    assert!(!sent.session.is_prepared());
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn discard_is_idempotent_releases_capability_and_does_not_delete_active_sessions() {
    let (dir, store, driver, app, config) = setup().await;
    let provider = Arc::new(FakeFeedbackProvider::default());
    let app = app.with_feedback_provider(provider.clone());
    let ready = app.prepare_session(prepare(&dir, &config)).await.unwrap();
    app.discard_prepared_session(target(&ready)).await.unwrap();
    app.discard_prepared_session(target(&ready)).await.unwrap();
    assert!(provider.bindings.lock().unwrap().is_empty());
    assert_eq!(
        driver.connections.lock().unwrap()[0]
            .stops
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        store.get_session(&ready.session.session_id).await,
        Err(SessionRepositoryError::SessionNotFound)
    );
    let active = app
        .create_session(input(&dir, &config, "Keep this title"))
        .await
        .unwrap();
    assert!(app.discard_prepared_session(target(&active)).await.is_err());
    assert_eq!(
        store
            .get_session(&active.session.session_id)
            .await
            .unwrap()
            .title,
        "Keep this title"
    );
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn first_send_and_discard_have_one_lifecycle_winner() {
    let (dir, store, driver, app, config) = setup().await;
    let ready = app.prepare_session(prepare(&dir, &config)).await.unwrap();
    let (sent, discarded) = tokio::join!(
        app.send_prompt(prompt(&ready)),
        app.discard_prepared_session(target(&ready))
    );
    match store.get_session(&ready.session.session_id).await {
        Ok(record) => {
            assert!(!record.is_prepared());
            assert!(sent.is_ok());
            assert!(discarded.is_err());
            assert!(
                !driver.connections.lock().unwrap()[0]
                    .closed
                    .load(Ordering::SeqCst)
            );
        }
        Err(SessionRepositoryError::SessionNotFound) => {
            assert!(sent.is_err());
            assert!(discarded.is_ok());
            assert_eq!(
                driver.connections.lock().unwrap()[0]
                    .prompts
                    .load(Ordering::SeqCst),
                0
            );
        }
        result => panic!("unexpected race outcome: {result:?}"),
    }
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn simultaneous_first_sends_do_not_publish_two_human_messages() {
    let (dir, store, _driver, app, config) = setup().await;
    let ready = app.prepare_session(prepare(&dir, &config)).await.unwrap();
    let (first, second) = tokio::join!(
        app.send_prompt(prompt(&ready)),
        app.send_prompt(prompt(&ready))
    );
    assert_ne!(first.is_ok(), second.is_ok());
    let activities = store
        .list_session_activity(&ready.session.session_id, None, 100)
        .await
        .unwrap();
    assert_eq!(
        activities
            .iter()
            .filter(|row| row.kind == SessionActivityKind::UserMessage)
            .count(),
        1
    );
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn fresh_runtime_removes_stale_prepared_records_without_touching_active_conversations() {
    let (dir, store, driver, app, config) = setup().await;
    let prepared = app.prepare_session(prepare(&dir, &config)).await.unwrap();
    let active = app
        .create_session(input(&dir, &config, "Existing conversation"))
        .await
        .unwrap();
    app.shutdown().await.unwrap();
    let fresh = SessionApplication::new(store.clone(), store.clone(), driver.clone());
    fresh.recover_runtime().await.unwrap();
    assert_eq!(
        store.get_session(&prepared.session.session_id).await,
        Err(SessionRepositoryError::SessionNotFound)
    );
    assert_eq!(
        store
            .get_session(&active.session.session_id)
            .await
            .unwrap()
            .title,
        "Existing conversation"
    );
    assert_eq!(driver.starts.lock().unwrap().len(), 2);
    let new_draft = fresh.prepare_session(prepare(&dir, &config)).await.unwrap();
    assert!(new_draft.session.is_prepared());
    fresh.get_session(target(&new_draft)).await.unwrap();
    fresh.shutdown().await.unwrap();
    store.close().await;
}
