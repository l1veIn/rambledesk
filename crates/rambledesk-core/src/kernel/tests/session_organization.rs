use super::super::ports::FactStoreError;
use super::{super::*, MemoryState};

pub(super) fn apply(
    state: &mut MemoryState,
    commit: SessionOrganizationCommit,
) -> Result<FactMutationOutcome, FactStoreError> {
    let session_id = commit.mutation.session_id().clone();
    let session = state
        .sessions
        .get(&session_id)
        .ok_or(FactStoreError::SessionNotFound)?;
    if session.kind != SessionKind::Managed {
        return Err(FactStoreError::SessionNotManaged);
    }
    if let SessionOrganization::SetArchived { archived: true, .. } = &commit.mutation {
        if session.archived_at.is_some() {
            return Ok(FactMutationOutcome::SessionOrganization(session.clone()));
        }
        let has_pending = state.requests.values().any(|request| {
            request.session_id == session_id && request.status == FeedbackRequestStatus::Waiting
        }) || state.deliveries.values().any(|delivery| {
            delivery.session_id == session_id && delivery.state == DeliveryState::Pending
        }) || state
            .work
            .values()
            .any(|work| work.session_id == session_id && work.state != AgentWorkState::Completed);
        if has_pending {
            return Err(FactStoreError::SessionHasPendingActivity);
        }
    }
    let session = state
        .sessions
        .get_mut(&session_id)
        .ok_or(FactStoreError::SessionNotFound)?;
    let changed = match commit.mutation {
        SessionOrganization::Rename { title, .. } if session.title != title => {
            session.title = title;
            true
        }
        SessionOrganization::SetPinned { pinned: true, .. } if session.pinned_at.is_none() => {
            session.pinned_at = Some(commit.now.clone());
            true
        }
        SessionOrganization::SetPinned { pinned: false, .. } if session.pinned_at.is_some() => {
            session.pinned_at = None;
            true
        }
        SessionOrganization::SetArchived { archived: true, .. }
            if session.archived_at.is_none() =>
        {
            session.archived_at = Some(commit.now.clone());
            true
        }
        SessionOrganization::SetArchived {
            archived: false, ..
        } if session.archived_at.is_some() => {
            session.archived_at = None;
            true
        }
        _ => false,
    };
    if changed {
        session.updated_at = commit.now;
    }
    Ok(FactMutationOutcome::SessionOrganization(session.clone()))
}

pub(super) fn archived(state: &MemoryState) -> Vec<SessionRecord> {
    let mut sessions = state
        .sessions
        .values()
        .filter(|session| session.archived_at.is_some())
        .cloned()
        .collect::<Vec<_>>();
    sessions.sort_by(|left, right| {
        right
            .archived_at
            .cmp(&left.archived_at)
            .then_with(|| right.session_id.cmp(&left.session_id))
    });
    sessions
}

#[tokio::test]
async fn organization_is_idempotent_and_archive_is_hidden_until_restored() {
    let (core, _) = test_fixtures::harness();
    let launch = test_fixtures::launch_session(&core).await;
    let session_id = launch.session_id;
    complete_launch_work(&core, &session_id).await;

    let renamed = core
        .organize_session(SessionOrganization::Rename {
            session_id: session_id.clone(),
            title: "Renamed Session".to_owned(),
        })
        .await
        .expect("rename");
    let replay = core
        .organize_session(SessionOrganization::Rename {
            session_id: session_id.clone(),
            title: "Renamed Session".to_owned(),
        })
        .await
        .expect("rename replay");
    assert_eq!(replay, renamed);

    let pinned = core
        .organize_session(SessionOrganization::SetPinned {
            session_id: session_id.clone(),
            pinned: true,
        })
        .await
        .expect("pin");
    let pin_replay = core
        .organize_session(SessionOrganization::SetPinned {
            session_id: session_id.clone(),
            pinned: true,
        })
        .await
        .expect("pin replay");
    assert_eq!(pin_replay.pinned_at, pinned.pinned_at);

    core.organize_session(SessionOrganization::SetArchived {
        session_id: session_id.clone(),
        archived: true,
    })
    .await
    .expect("archive");
    assert!(
        core.read_workbench(WorkbenchQuery { session_id: None })
            .await
            .expect("active")
            .sessions
            .is_empty()
    );
    assert_eq!(
        core.read_archived_sessions().await.expect("archived").len(),
        1
    );

    core.organize_session(SessionOrganization::SetArchived {
        session_id,
        archived: false,
    })
    .await
    .expect("restore");
    assert_eq!(
        core.read_workbench(WorkbenchQuery { session_id: None })
            .await
            .expect("restored")
            .sessions
            .len(),
        1
    );
}

#[tokio::test]
async fn archive_rejects_pending_feedback_or_agent_work() {
    let (core, _) = test_fixtures::harness();
    let launch = test_fixtures::launch_session(&core).await;
    let error = core
        .organize_session(SessionOrganization::SetArchived {
            session_id: launch.session_id.clone(),
            archived: true,
        })
        .await
        .expect_err("pending launch work");
    assert_eq!(error.code(), CoreErrorCode::SessionHasPendingActivity);

    complete_launch_work(&core, &launch.session_id).await;
    test_fixtures::create_request(&core, launch.session_id.clone(), "pending-feedback").await;
    let error = core
        .organize_session(SessionOrganization::SetArchived {
            session_id: launch.session_id,
            archived: true,
        })
        .await
        .expect_err("pending Feedback");
    assert_eq!(error.code(), CoreErrorCode::SessionHasPendingActivity);
}

async fn complete_launch_work(core: &Core, session_id: &SessionId) {
    let batch = core
        .claim_agent_work(WorkScope {
            session_id: Some(session_id.clone()),
            limit: 1,
            lease_seconds: 60,
        })
        .await
        .expect("claim launch");
    core.record_agent_work(AgentWorkResult {
        work_id: batch.items[0].work.work_id.clone(),
        claim_token: batch.items[0].claim_token.clone(),
        disposition: AgentWorkDisposition::Completed {
            evidence: AgentWorkEvidence::PromptTurnCompleted,
        },
    })
    .await
    .expect("complete launch");
}
