use std::path::Path;

use rambledesk_core::kernel::ports::{FactStore, FactStoreError};
use rambledesk_core::kernel::*;
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use tempfile::TempDir;

use super::{SqliteV3OpenError, SqliteV3Store};

mod recovery;
mod schema;
mod vertical;

const NOW: &str = "2026-08-30T00:00:00Z";
const LATER: &str = "2026-08-30T00:01:00Z";

fn digest(value: char) -> String {
    format!("sha256:{}", value.to_string().repeat(64))
}

fn managed_session(id: &str) -> SessionRecord {
    SessionRecord {
        session_id: SessionId::new(id),
        kind: SessionKind::Managed,
        title: "Managed session".to_owned(),
        lifecycle: SessionLifecycle::Ready,
        launch_configuration: Some(LaunchConfiguration {
            agent_profile_id: "codex".to_owned(),
            launch_profile_id: "local".to_owned(),
            workspace_reference: "/workspace".to_owned(),
            model: Some("gpt-5".to_owned()),
            reasoning_effort: Some("high".to_owned()),
            access_mode: AccessMode::WorkspaceWrite,
            agent_config_json: "opaque-agent-config".to_owned(),
        }),
        created_at: NOW.to_owned(),
        updated_at: NOW.to_owned(),
    }
}

fn submission(
    id: &str,
    session_id: &str,
    intent: RambleIntent,
    request_id: Option<&str>,
    submission_digest: String,
) -> RambleSubmissionRecord {
    RambleSubmissionRecord {
        submission_id: SubmissionId::new(id),
        session_id: SessionId::new(session_id),
        intent,
        request_id: request_id.map(RequestId::new),
        document_json: "opaque-document".to_owned(),
        body_markdown: format!("body-{id}"),
        submission_digest,
        artifacts: Vec::new(),
        created_at: NOW.to_owned(),
    }
}

fn package(
    id: &str,
    submission_id: &str,
    purpose: PackagePurpose,
    request_id: Option<&str>,
) -> PackageRecord {
    let package_id = PackageId::new(id);
    let submission_id = SubmissionId::new(submission_id);
    let request_id = request_id.map(RequestId::new);
    let artifacts = Vec::new();
    let digests = calculate_package_digests(PackageDigestInput {
        package_id: &package_id,
        submission_id: &submission_id,
        purpose,
        request_id: request_id.as_ref(),
        schema_version: 3,
        artifacts: &artifacts,
        published_at: NOW,
    });
    PackageRecord {
        package_id,
        submission_id,
        purpose,
        request_id,
        content_digest: digests.content_digest,
        manifest_digest: digests.manifest_digest,
        schema_version: 3,
        artifacts,
        published_at: NOW.to_owned(),
    }
}

fn prompt_work(
    id: &str,
    session_id: &str,
    submission_id: &str,
    package_id: &str,
) -> AgentWorkRecord {
    AgentWorkRecord {
        work_id: AgentWorkId::new(id),
        session_id: SessionId::new(session_id),
        kind: AgentWorkKind::LaunchPrompt,
        source_id: submission_id.to_owned(),
        payload_digest: digest('e'),
        payload: AgentWorkPayload::Launch {
            submission_id: SubmissionId::new(submission_id),
            package_id: PackageId::new(package_id),
            prompt_markdown: format!("body-{submission_id}"),
        },
        state: AgentWorkState::Pending,
        attempt_count: 0,
        last_error_code: None,
        last_error_at: None,
        created_at: NOW.to_owned(),
        completed_at: None,
    }
}

fn launch_commit(submission_digest: String) -> LaunchCommit {
    let session = managed_session("session-1");
    let submission = submission(
        "launch-1",
        session.session_id.as_str(),
        RambleIntent::Launch,
        None,
        submission_digest.clone(),
    );
    let package = package(
        "package-launch-1",
        submission.submission_id.as_str(),
        PackagePurpose::Launch,
        None,
    );
    let work = prompt_work(
        "work-launch-1",
        session.session_id.as_str(),
        submission.submission_id.as_str(),
        package.package_id.as_str(),
    );
    LaunchCommit {
        outcome: LaunchOutcome {
            session_id: session.session_id.clone(),
            submission_id: submission.submission_id.clone(),
            submission_digest,
            package_id: package.package_id.clone(),
            package_content_digest: package.content_digest.clone(),
            package_manifest_digest: package.manifest_digest.clone(),
            agent_work_id: work.work_id.clone(),
            agent_work_state: AgentWorkState::Pending,
        },
        session,
        submission,
        package,
        work,
    }
}

fn waiting_request(id: &str, session_id: &str) -> FeedbackRequestSnapshot {
    FeedbackRequestSnapshot {
        request_id: RequestId::new(id),
        session_id: SessionId::new(session_id),
        source_link_id: None,
        title: "Review".to_owned(),
        instructions: "Review the work".to_owned(),
        actions: Vec::new(),
        context_refs: Vec::new(),
        input_digest: digest('a'),
        status: FeedbackRequestStatus::Waiting,
        resolution: None,
        response_package_id: None,
        cancel_reason: None,
        request_artifacts: Vec::new(),
        created_at: NOW.to_owned(),
        resolved_at: None,
    }
}

fn feedback_work(
    id: &str,
    session_id: &str,
    request_id: &str,
    delivery_id: &str,
) -> AgentWorkRecord {
    AgentWorkRecord {
        work_id: AgentWorkId::new(id),
        session_id: SessionId::new(session_id),
        kind: AgentWorkKind::FeedbackResume,
        source_id: delivery_id.to_owned(),
        payload_digest: digest('f'),
        payload: AgentWorkPayload::FeedbackResume {
            delivery_id: DeliveryId::new(delivery_id),
            request_id: RequestId::new(request_id),
        },
        state: AgentWorkState::Pending,
        attempt_count: 0,
        last_error_code: None,
        last_error_at: None,
        created_at: LATER.to_owned(),
        completed_at: None,
    }
}

fn cancel_commit(request: FeedbackRequestSnapshot, suffix: &str) -> FeedbackResolutionCommit {
    let mut terminal = request.clone();
    terminal.status = FeedbackRequestStatus::Cancelled;
    terminal.resolution = Some(FeedbackResolution::Cancelled);
    terminal.cancel_reason = Some("No longer needed".to_owned());
    terminal.resolved_at = Some(LATER.to_owned());
    let delivery_id = format!("delivery-{suffix}");
    let work = feedback_work(
        &format!("work-{suffix}"),
        request.session_id.as_str(),
        request.request_id.as_str(),
        &delivery_id,
    );
    FeedbackResolutionCommit {
        request_id: request.request_id.clone(),
        expected_draft_revision: None,
        submission: None,
        package: None,
        resolution: FeedbackResolution::Cancelled,
        cancel_reason: terminal.cancel_reason.clone(),
        delivery: FeedbackDeliveryRecord {
            delivery_id: DeliveryId::new(&delivery_id),
            request_id: request.request_id.clone(),
            session_id: request.session_id.clone(),
            resolution: FeedbackResolution::Cancelled,
            package: None,
            cancel_reason: terminal.cancel_reason.clone(),
            state: DeliveryState::Pending,
            attempt_count: 0,
            last_error_code: None,
            last_error_at: None,
            created_at: LATER.to_owned(),
            delivered_at: None,
        },
        work: work.clone(),
        outcome: FeedbackResolutionOutcome {
            request: terminal,
            package_id: None,
            package_content_digest: None,
            package_manifest_digest: None,
            delivery_id: DeliveryId::new(delivery_id),
            delivery_state: DeliveryState::Pending,
            agent_work_id: work.work_id,
        },
    }
}

async fn open(temp: &TempDir) -> SqliteV3Store {
    SqliteV3Store::connect(&temp.path().join("facts.sqlite3"))
        .await
        .expect("open v3 store")
}

async fn seed_launch(store: &SqliteV3Store) -> LaunchOutcome {
    match store
        .apply(FactMutation::Launch(Box::new(launch_commit(digest('a')))))
        .await
        .expect("launch")
    {
        FactMutationOutcome::Launch(value) => value,
        _ => panic!("wrong launch outcome"),
    }
}

#[tokio::test]
async fn launch_replay_is_stable_and_changed_digest_conflicts() {
    let temp = TempDir::new().expect("tempdir");
    let store = open(&temp).await;
    let first = seed_launch(&store).await;
    let replay = store
        .apply(FactMutation::Launch(Box::new(launch_commit(digest('a')))))
        .await
        .expect("launch replay");
    assert_eq!(replay, FactMutationOutcome::Launch(first));
    let error = store
        .apply(FactMutation::Launch(Box::new(launch_commit(digest('b')))))
        .await
        .expect_err("changed digest");
    assert_eq!(error, FactStoreError::IdempotencyConflict);
}

#[tokio::test]
async fn restart_restores_waiting_draft_and_submitted_pending_delivery() {
    let temp = TempDir::new().expect("tempdir");
    let store = open(&temp).await;
    let launch = seed_launch(&store).await;
    let waiting = waiting_request("request-waiting", launch.session_id.as_str());
    store
        .apply(FactMutation::FeedbackRequest(Box::new(
            FeedbackRequestCommit {
                request: waiting.clone(),
            },
        )))
        .await
        .expect("waiting request");
    let draft = SaveDraft {
        draft_id: DraftId::new("draft-waiting"),
        intent: RambleIntent::Feedback,
        session_id: Some(launch.session_id.clone()),
        request_id: Some(waiting.request_id.clone()),
        launch_configuration: None,
        document_json: "opaque-draft".to_owned(),
        body_markdown: "draft body".to_owned(),
        expected_revision: 0,
    };
    store
        .apply(FactMutation::Draft(Box::new(DraftCommit {
            mutation: StoredDraftMutation::Save(draft.clone()),
            now: NOW.to_owned(),
        })))
        .await
        .expect("save draft");
    let stale = store
        .apply(FactMutation::Draft(Box::new(DraftCommit {
            mutation: StoredDraftMutation::Save(draft),
            now: LATER.to_owned(),
        })))
        .await
        .expect_err("draft CAS");
    assert_eq!(stale, FactStoreError::DraftConflict);

    let submitted = waiting_request("request-submitted", launch.session_id.as_str());
    store
        .apply(FactMutation::FeedbackRequest(Box::new(
            FeedbackRequestCommit {
                request: submitted.clone(),
            },
        )))
        .await
        .expect("submitted request");
    let submission = submission(
        "feedback-submission",
        launch.session_id.as_str(),
        RambleIntent::Feedback,
        Some(submitted.request_id.as_str()),
        digest('b'),
    );
    let package = package(
        "package-response",
        submission.submission_id.as_str(),
        PackagePurpose::Response,
        Some(submitted.request_id.as_str()),
    );
    let mut terminal = submitted.clone();
    terminal.status = FeedbackRequestStatus::Submitted;
    terminal.resolution = Some(FeedbackResolution::Submitted);
    terminal.response_package_id = Some(package.package_id.clone());
    terminal.resolved_at = Some(LATER.to_owned());
    let delivery_id = DeliveryId::new("delivery-submitted");
    let work = feedback_work(
        "work-submitted",
        launch.session_id.as_str(),
        submitted.request_id.as_str(),
        delivery_id.as_str(),
    );
    let delivery = FeedbackDeliveryRecord {
        delivery_id: delivery_id.clone(),
        request_id: submitted.request_id.clone(),
        session_id: launch.session_id.clone(),
        resolution: FeedbackResolution::Submitted,
        package: Some(package.clone()),
        cancel_reason: None,
        state: DeliveryState::Pending,
        attempt_count: 0,
        last_error_code: None,
        last_error_at: None,
        created_at: LATER.to_owned(),
        delivered_at: None,
    };
    store
        .apply(FactMutation::FeedbackResolution(Box::new(
            FeedbackResolutionCommit {
                request_id: submitted.request_id.clone(),
                expected_draft_revision: Some(0),
                submission: Some(submission),
                package: Some(package.clone()),
                resolution: FeedbackResolution::Submitted,
                cancel_reason: None,
                delivery: delivery.clone(),
                work: work.clone(),
                outcome: FeedbackResolutionOutcome {
                    request: terminal,
                    package_id: Some(package.package_id.clone()),
                    package_content_digest: Some(package.content_digest.clone()),
                    package_manifest_digest: Some(package.manifest_digest.clone()),
                    delivery_id,
                    delivery_state: DeliveryState::Pending,
                    agent_work_id: work.work_id.clone(),
                },
            },
        )))
        .await
        .expect("offline submit");
    store.close().await;

    let reopened = open(&temp).await;
    let snapshot = match reopened
        .query(FactQuery::Workbench(WorkbenchQuery { session_id: None }))
        .await
        .expect("restart snapshot")
    {
        FactQueryOutcome::Workbench(value) => value,
        _ => panic!("wrong snapshot"),
    };
    assert_eq!(snapshot.waiting_feedback, vec![waiting]);
    assert_eq!(snapshot.drafts.len(), 1);
    assert_eq!(snapshot.drafts[0].document_json, "opaque-draft");
    assert_eq!(snapshot.pending_deliveries, vec![delivery]);
    assert!(
        snapshot
            .pending_agent_work
            .iter()
            .any(|value| value.work_id == work.work_id)
    );
}

#[tokio::test]
async fn lease_expiry_stale_token_and_feedback_completion_are_atomic() {
    let temp = TempDir::new().expect("tempdir");
    let store = open(&temp).await;
    let launch = seed_launch(&store).await;
    let scope = WorkScope {
        session_id: Some(launch.session_id.clone()),
        limit: 1,
        lease_seconds: 30,
    };
    let first = store
        .claim_work(WorkClaim {
            scope: scope.clone(),
            claim_token: WorkClaimToken::new("token-1"),
            claimed_at: NOW.to_owned(),
            lease_until: LATER.to_owned(),
        })
        .await
        .expect("claim");
    assert_eq!(first.items.len(), 1);
    let reclaimed = store
        .claim_work(WorkClaim {
            scope: scope.clone(),
            claim_token: WorkClaimToken::new("token-2"),
            claimed_at: "2026-08-30T00:02:00Z".to_owned(),
            lease_until: "2026-08-30T00:03:00Z".to_owned(),
        })
        .await
        .expect("reclaim expired");
    assert_eq!(reclaimed.items.len(), 1);
    let work_id = reclaimed.items[0].work.work_id.clone();
    let stale = store
        .record_work(StoredWorkResult {
            result: AgentWorkResult {
                work_id: work_id.clone(),
                claim_token: WorkClaimToken::new("token-1"),
                disposition: AgentWorkDisposition::Completed {
                    evidence: AgentWorkEvidence::PromptTurnCompleted,
                },
            },
            recorded_at: LATER.to_owned(),
        })
        .await
        .expect_err("stale token");
    assert_eq!(stale, FactStoreError::WorkClaimConflict);
    store
        .record_work(StoredWorkResult {
            result: AgentWorkResult {
                work_id,
                claim_token: WorkClaimToken::new("token-2"),
                disposition: AgentWorkDisposition::Completed {
                    evidence: AgentWorkEvidence::PromptTurnCompleted,
                },
            },
            recorded_at: LATER.to_owned(),
        })
        .await
        .expect("complete prompt");

    let request = waiting_request("request-complete", launch.session_id.as_str());
    store
        .apply(FactMutation::FeedbackRequest(Box::new(
            FeedbackRequestCommit {
                request: request.clone(),
            },
        )))
        .await
        .expect("request");
    let commit = cancel_commit(request, "complete");
    let delivery_id = commit.delivery.delivery_id.clone();
    store
        .apply(FactMutation::FeedbackResolution(Box::new(commit)))
        .await
        .expect("cancel");
    let batch = store
        .claim_work(WorkClaim {
            scope: scope.clone(),
            claim_token: WorkClaimToken::new("feedback-token"),
            claimed_at: NOW.to_owned(),
            lease_until: LATER.to_owned(),
        })
        .await
        .expect("claim feedback");
    assert_eq!(batch.items.len(), 1);
    let feedback_work_id = batch.items[0].work.work_id.clone();
    let retry = store
        .record_work(StoredWorkResult {
            result: AgentWorkResult {
                work_id: feedback_work_id.clone(),
                claim_token: WorkClaimToken::new("feedback-token"),
                disposition: AgentWorkDisposition::Retry {
                    error_code: "ACP_DISCONNECTED".to_owned(),
                },
            },
            recorded_at: "2026-08-30T00:00:30Z".to_owned(),
        })
        .await
        .expect("record retry");
    assert_eq!(retry.state, AgentWorkState::Pending);
    let retry_row: (i64, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT attempt_count, last_error_code, lease_token FROM agent_work_v3 WHERE work_id = ?",
    )
    .bind(feedback_work_id.as_str())
    .fetch_one(&store.pool)
    .await
    .expect("retry diagnostics");
    assert_eq!(retry_row, (1, Some("ACP_DISCONNECTED".to_owned()), None));
    let batch = store
        .claim_work(WorkClaim {
            scope,
            claim_token: WorkClaimToken::new("feedback-token-2"),
            claimed_at: "2026-08-30T00:00:40Z".to_owned(),
            lease_until: LATER.to_owned(),
        })
        .await
        .expect("reclaim feedback");
    assert_eq!(batch.items.len(), 1);
    assert_eq!(batch.items[0].work.attempt_count, 2);
    let delivery_attempts: (i64, Option<String>) = sqlx::query_as(
        "SELECT attempt_count, last_error_code FROM feedback_deliveries_v3 WHERE delivery_id = ?",
    )
    .bind(delivery_id.as_str())
    .fetch_one(&store.pool)
    .await
    .expect("delivery retry diagnostics");
    assert_eq!(delivery_attempts, (2, Some("ACP_DISCONNECTED".to_owned())));
    let expired = store
        .record_work(StoredWorkResult {
            result: AgentWorkResult {
                work_id: batch.items[0].work.work_id.clone(),
                claim_token: WorkClaimToken::new("feedback-token-2"),
                disposition: AgentWorkDisposition::Completed {
                    evidence: AgentWorkEvidence::FeedbackConsumedAndTurnCompleted {
                        delivery_id: delivery_id.clone(),
                    },
                },
            },
            recorded_at: LATER.to_owned(),
        })
        .await
        .expect_err("lease boundary is expired");
    assert_eq!(expired, FactStoreError::WorkClaimConflict);
    let completed = store
        .record_work(StoredWorkResult {
            result: AgentWorkResult {
                work_id: batch.items[0].work.work_id.clone(),
                claim_token: WorkClaimToken::new("feedback-token-2"),
                disposition: AgentWorkDisposition::Completed {
                    evidence: AgentWorkEvidence::FeedbackConsumedAndTurnCompleted {
                        delivery_id: delivery_id.clone(),
                    },
                },
            },
            recorded_at: "2026-08-30T00:00:59Z".to_owned(),
        })
        .await
        .expect("complete feedback");
    assert_eq!(completed.delivered, Some(delivery_id.clone()));
    let delivery_state: String =
        sqlx::query_scalar("SELECT state FROM feedback_deliveries_v3 WHERE delivery_id = ?")
            .bind(delivery_id.as_str())
            .fetch_one(&store.pool)
            .await
            .expect("delivery state");
    assert_eq!(delivery_state, "delivered");
}

#[tokio::test]
async fn legacy_preflight_is_strictly_read_only() {
    let temp = TempDir::new().expect("tempdir");
    let path = temp.path().join("legacy.sqlite3");
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .expect("legacy connection");
    sqlx::query("CREATE TABLE host_sessions (id TEXT PRIMARY KEY)")
        .execute(&mut connection)
        .await
        .expect("legacy schema");
    connection.close().await.expect("close legacy");
    let before_bytes = tokio::fs::read(&path).await.expect("legacy bytes");
    let before_modified = tokio::fs::metadata(&path)
        .await
        .expect("legacy metadata")
        .modified()
        .expect("modified");

    let error = match SqliteV3Store::connect(&path).await {
        Ok(_) => panic!("legacy must be rejected"),
        Err(error) => error,
    };
    assert!(matches!(error, SqliteV3OpenError::LegacyDatabaseRejected));
    assert_eq!(
        tokio::fs::read(&path).await.expect("bytes after"),
        before_bytes
    );
    assert_eq!(
        tokio::fs::metadata(&path)
            .await
            .expect("metadata after")
            .modified()
            .expect("modified after"),
        before_modified
    );
    assert!(!sidecar(&path, "-wal").exists());
    assert!(!sidecar(&path, "-shm").exists());

    let mut probe = SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(&path).read_only(true),
    )
    .await
    .expect("read-only probe");
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name LIKE '%\\_v3' ESCAPE '\\'",
    )
    .fetch_one(&mut probe)
    .await
    .expect("v3 tables");
    assert_eq!(count, 0);
}

fn sidecar(path: &Path, suffix: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{}{}", path.display(), suffix))
}

#[tokio::test]
async fn connected_feedback_resolution_is_rejected_by_storage() {
    let temp = TempDir::new().expect("tempdir");
    let store = open(&temp).await;
    sqlx::query(
        "INSERT INTO sessions_v3 (
            session_id, session_kind, title, lifecycle, launch_configuration_json,
            created_at, updated_at
         ) VALUES ('connected-1', 'connected', 'Migrated', 'ready', NULL, ?, ?)",
    )
    .bind(NOW)
    .bind(NOW)
    .execute(&store.pool)
    .await
    .expect("connected session");
    let request = waiting_request("request-connected", "connected-1");
    store
        .apply(FactMutation::FeedbackRequest(Box::new(
            FeedbackRequestCommit {
                request: request.clone(),
            },
        )))
        .await
        .expect("connected request");
    let error = store
        .apply(FactMutation::FeedbackResolution(Box::new(cancel_commit(
            request.clone(),
            "connected",
        ))))
        .await
        .expect_err("connected resolution");
    assert_eq!(error, FactStoreError::SessionNotManaged);
    let pending: (i64, i64) = sqlx::query_as(
        "SELECT
            (SELECT COUNT(*) FROM feedback_requests_v3 WHERE request_id = ? AND resolution IS NULL),
            (SELECT COUNT(*) FROM agent_work_v3 WHERE session_id = 'connected-1')",
    )
    .bind(request.request_id.as_str())
    .fetch_one(&store.pool)
    .await
    .expect("connected facts");
    assert_eq!(pending, (1, 0));
}

#[tokio::test]
async fn missing_source_link_maps_to_domain_error_and_checkpoint_refreshes() {
    let temp = TempDir::new().expect("tempdir");
    let store = open(&temp).await;
    let launch = seed_launch(&store).await;
    let mut request = waiting_request("request-missing-link", launch.session_id.as_str());
    request.source_link_id = Some(AcpSessionLinkId::new("unknown-link"));
    let error = store
        .apply(FactMutation::FeedbackRequest(Box::new(
            FeedbackRequestCommit { request },
        )))
        .await
        .expect_err("missing link");
    assert_eq!(error, FactStoreError::AcpSessionLinkNotFound);

    let observation = AgentObservation::AcpSessionLinked(AcpSessionLinkObservation {
        session_id: launch.session_id.clone(),
        agent_profile_id: "codex".to_owned(),
        launch_profile_id: "local".to_owned(),
        acp_session_id: "acp-1".to_owned(),
        capabilities_json: "opaque-capabilities-1".to_owned(),
        session_toolset_digest: digest('1'),
    });
    let first_link = AcpSessionLinkSnapshot {
        link_id: AcpSessionLinkId::new("link-1"),
        session_id: launch.session_id.clone(),
        agent_profile_id: "codex".to_owned(),
        launch_profile_id: "local".to_owned(),
        acp_session_id: "acp-1".to_owned(),
        capabilities_json: "opaque-capabilities-1".to_owned(),
        session_toolset_digest: digest('1'),
        is_current: true,
        created_at: NOW.to_owned(),
        last_used_at: NOW.to_owned(),
    };
    store
        .apply(FactMutation::AgentObservation(Box::new(
            AgentObservationCommit {
                observation,
                link: first_link.clone(),
            },
        )))
        .await
        .expect("first link");
    let refreshed = store
        .apply(FactMutation::AgentObservation(Box::new(
            AgentObservationCommit {
                observation: AgentObservation::AcpSessionLinked(AcpSessionLinkObservation {
                    session_id: launch.session_id,
                    agent_profile_id: "codex".to_owned(),
                    launch_profile_id: "local".to_owned(),
                    acp_session_id: "acp-1".to_owned(),
                    capabilities_json: "opaque-capabilities-2".to_owned(),
                    session_toolset_digest: digest('2'),
                }),
                link: AcpSessionLinkSnapshot {
                    link_id: AcpSessionLinkId::new("new-link-must-not-win"),
                    capabilities_json: "opaque-capabilities-2".to_owned(),
                    session_toolset_digest: digest('2'),
                    last_used_at: LATER.to_owned(),
                    ..first_link.clone()
                },
            },
        )))
        .await
        .expect("refresh link");
    let FactMutationOutcome::AgentObservation(refreshed) = refreshed else {
        panic!("wrong observation outcome")
    };
    assert_eq!(refreshed.link_id, first_link.link_id);
    assert_eq!(refreshed.created_at, NOW);
    assert_eq!(refreshed.last_used_at, LATER);
    assert_eq!(refreshed.capabilities_json, "opaque-capabilities-2");
}
