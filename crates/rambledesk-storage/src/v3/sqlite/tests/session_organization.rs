use super::*;

#[tokio::test]
async fn organization_survives_restart_and_orders_pinned_sessions_stably() {
    let temp = TempDir::new().expect("tempdir");
    let store = open(&temp).await;
    insert_managed_session(&store, "session-a", "Alpha").await;
    insert_managed_session(&store, "session-b", "Beta").await;

    let renamed = organize(
        &store,
        SessionOrganization::Rename {
            session_id: SessionId::new("session-a"),
            title: "Renamed".to_owned(),
        },
        LATER,
    )
    .await;
    let replay = organize(
        &store,
        SessionOrganization::Rename {
            session_id: SessionId::new("session-a"),
            title: "Renamed".to_owned(),
        },
        "2026-08-30T00:02:00Z",
    )
    .await;
    assert_eq!(replay.updated_at, renamed.updated_at);

    let pinned = organize(
        &store,
        SessionOrganization::SetPinned {
            session_id: SessionId::new("session-a"),
            pinned: true,
        },
        "2026-08-30T00:03:00Z",
    )
    .await;
    let pin_replay = organize(
        &store,
        SessionOrganization::SetPinned {
            session_id: SessionId::new("session-a"),
            pinned: true,
        },
        "2026-08-30T00:04:00Z",
    )
    .await;
    assert_eq!(pin_replay.pinned_at, pinned.pinned_at);

    let active = workbench(&store).await;
    assert_eq!(
        active
            .sessions
            .iter()
            .map(|session| session.session_id.as_str())
            .collect::<Vec<_>>(),
        vec!["session-a", "session-b"]
    );

    organize(
        &store,
        SessionOrganization::SetArchived {
            session_id: SessionId::new("session-a"),
            archived: true,
        },
        "2026-08-30T00:05:00Z",
    )
    .await;
    assert_eq!(workbench(&store).await.sessions.len(), 1);
    store.close().await;

    let reopened = open(&temp).await;
    let archived_sessions = archived(&reopened).await;
    assert_eq!(archived_sessions.len(), 1);
    assert_eq!(archived_sessions[0].title, "Renamed");
    assert_eq!(archived_sessions[0].pinned_at, pinned.pinned_at);
    organize(
        &reopened,
        SessionOrganization::SetArchived {
            session_id: SessionId::new("session-a"),
            archived: false,
        },
        "2026-08-30T00:06:00Z",
    )
    .await;
    assert_eq!(workbench(&reopened).await.sessions.len(), 2);
    assert!(archived(&reopened).await.is_empty());
}

#[tokio::test]
async fn archive_rejects_pending_feedback_and_agent_work_atomically() {
    let temp = TempDir::new().expect("tempdir");
    let store = open(&temp).await;
    let launch = seed_launch(&store).await;
    let error = store
        .apply(FactMutation::SessionOrganization(Box::new(
            SessionOrganizationCommit {
                mutation: SessionOrganization::SetArchived {
                    session_id: launch.session_id.clone(),
                    archived: true,
                },
                now: LATER.to_owned(),
            },
        )))
        .await
        .expect_err("pending launch work");
    assert_eq!(error, FactStoreError::SessionHasPendingActivity);

    insert_managed_session(&store, "feedback-session", "Feedback").await;
    store
        .apply(FactMutation::FeedbackRequest(Box::new(
            FeedbackRequestCommit {
                request: waiting_request("pending-feedback", "feedback-session"),
            },
        )))
        .await
        .expect("waiting Feedback");
    let error = store
        .apply(FactMutation::SessionOrganization(Box::new(
            SessionOrganizationCommit {
                mutation: SessionOrganization::SetArchived {
                    session_id: SessionId::new("feedback-session"),
                    archived: true,
                },
                now: LATER.to_owned(),
            },
        )))
        .await
        .expect_err("pending Feedback");
    assert_eq!(error, FactStoreError::SessionHasPendingActivity);
}

async fn insert_managed_session(store: &SqliteV3Store, id: &str, title: &str) {
    let mut session = managed_session(id);
    session.title = title.to_owned();
    let mut transaction = store.pool.begin().await.expect("begin");
    super::super::write_support::insert_session(transaction.as_mut(), &session)
        .await
        .expect("insert session");
    transaction.commit().await.expect("commit");
}

async fn organize(
    store: &SqliteV3Store,
    mutation: SessionOrganization,
    now: &str,
) -> SessionRecord {
    let outcome = store
        .apply(FactMutation::SessionOrganization(Box::new(
            SessionOrganizationCommit {
                mutation,
                now: now.to_owned(),
            },
        )))
        .await
        .expect("organize");
    let FactMutationOutcome::SessionOrganization(session) = outcome else {
        panic!("wrong organization outcome")
    };
    session
}

async fn workbench(store: &SqliteV3Store) -> WorkbenchSnapshot {
    let outcome = store
        .query(FactQuery::Workbench(WorkbenchQuery { session_id: None }))
        .await
        .expect("workbench");
    let FactQueryOutcome::Workbench(snapshot) = outcome else {
        panic!("wrong workbench outcome")
    };
    snapshot
}

async fn archived(store: &SqliteV3Store) -> Vec<SessionRecord> {
    let outcome = store
        .query(FactQuery::ArchivedSessions)
        .await
        .expect("archived sessions");
    let FactQueryOutcome::ArchivedSessions(sessions) = outcome else {
        panic!("wrong archived outcome")
    };
    sessions
}
