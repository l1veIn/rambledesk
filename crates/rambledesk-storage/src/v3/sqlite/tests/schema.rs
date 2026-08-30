use super::*;

#[tokio::test]
async fn existing_empty_database_is_initialized_as_v3() {
    let temp = TempDir::new().expect("tempdir");
    let path = temp.path().join("empty.sqlite3");
    tokio::fs::write(&path, b"").await.expect("empty file");
    let store = SqliteV3Store::connect(&path).await.expect("initialize v3");
    let generation: i64 =
        sqlx::query_scalar("SELECT generation FROM schema_generation_v3 WHERE singleton = 1")
            .fetch_one(&store.pool)
            .await
            .expect("generation");
    assert_eq!(generation, 3);
}

#[tokio::test]
async fn unknown_database_is_rejected_without_mutation() {
    let temp = TempDir::new().expect("tempdir");
    let path = temp.path().join("foreign.sqlite3");
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .expect("foreign connection");
    sqlx::query("CREATE TABLE unrelated_product (id TEXT PRIMARY KEY)")
        .execute(&mut connection)
        .await
        .expect("foreign schema");
    connection.close().await.expect("close foreign");
    let before = tokio::fs::read(&path).await.expect("foreign bytes");
    let modified = tokio::fs::metadata(&path)
        .await
        .expect("foreign metadata")
        .modified()
        .expect("modified");
    let error = match SqliteV3Store::connect(&path).await {
        Ok(_) => panic!("unknown database must be rejected"),
        Err(error) => error,
    };
    assert!(matches!(error, SqliteV3OpenError::UnknownDatabaseRejected));
    assert_eq!(tokio::fs::read(&path).await.expect("after bytes"), before);
    assert_eq!(
        tokio::fs::metadata(&path)
            .await
            .expect("after metadata")
            .modified()
            .expect("after modified"),
        modified
    );
    assert!(!sidecar(&path, "-wal").exists());
    assert!(!sidecar(&path, "-shm").exists());
}

#[tokio::test]
async fn familiar_v3_table_without_migration_identity_is_still_rejected() {
    let temp = TempDir::new().expect("tempdir");
    let path = temp.path().join("lookalike.sqlite3");
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .expect("lookalike connection");
    sqlx::query(
        "CREATE TABLE schema_generation_v3 (
            singleton INTEGER PRIMARY KEY,
            generation INTEGER NOT NULL,
            revision INTEGER NOT NULL
         )",
    )
    .execute(&mut connection)
    .await
    .expect("lookalike marker");
    connection.close().await.expect("close lookalike");
    let before = tokio::fs::read(&path).await.expect("lookalike bytes");

    let error = match SqliteV3Store::connect(&path).await {
        Ok(_) => panic!("partial v3 lookalike must be rejected"),
        Err(error) => error,
    };

    assert!(matches!(error, SqliteV3OpenError::UnknownDatabaseRejected));
    assert_eq!(tokio::fs::read(&path).await.expect("after bytes"), before);
    assert!(!sidecar(&path, "-wal").exists());
    assert!(!sidecar(&path, "-shm").exists());
}

#[tokio::test]
async fn package_digest_mismatch_is_rejected_before_any_facts_commit() {
    let temp = TempDir::new().expect("tempdir");
    let store = open(&temp).await;
    let mut launch = launch_commit(digest('a'));
    launch.package.manifest_digest = digest('9');

    let error = store
        .apply(FactMutation::Launch(Box::new(launch)))
        .await
        .expect_err("non-canonical package digest");

    assert_eq!(error, FactStoreError::CorruptData);
    let session_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions_v3")
        .fetch_one(&store.pool)
        .await
        .expect("session count");
    assert_eq!(session_count, 0, "the rejected aggregate must roll back");
}

#[tokio::test]
async fn immutable_package_aggregate_rejects_direct_sql_drift() {
    let temp = TempDir::new().expect("tempdir");
    let store = open(&temp).await;
    let mut launch = launch_commit(digest('a'));
    launch.submission.artifacts.push(SubmissionArtifact {
        artifact_id: ArtifactId::new("artifact-submission"),
        position: 0,
        display_name: "entry".to_owned(),
        media_type: "text/plain".to_owned(),
        size_bytes: 1,
        sha256: digest('8'),
        storage_key: "test-object".to_owned(),
    });
    launch.package.artifacts.push(PackageArtifact {
        artifact_id: ArtifactId::new("artifact-package"),
        role: ArtifactRole::Attachment,
        position: 0,
        display_name: "entry".to_owned(),
        media_type: "text/plain".to_owned(),
        size_bytes: 1,
        sha256: digest('8'),
        storage_key: "test-object".to_owned(),
    });
    let package_digests = calculate_package_digests(PackageDigestInput {
        package_id: &launch.package.package_id,
        submission_id: &launch.package.submission_id,
        purpose: launch.package.purpose,
        request_id: launch.package.request_id.as_ref(),
        schema_version: launch.package.schema_version,
        artifacts: &launch.package.artifacts,
        published_at: &launch.package.published_at,
    });
    launch.package.content_digest = package_digests.content_digest.clone();
    launch.package.manifest_digest = package_digests.manifest_digest.clone();
    launch.outcome.package_content_digest = package_digests.content_digest;
    launch.outcome.package_manifest_digest = package_digests.manifest_digest;
    store
        .apply(FactMutation::Launch(Box::new(launch)))
        .await
        .expect("aggregate launch");
    let update = sqlx::query(
        "UPDATE packages_v3 SET manifest_digest = ? WHERE package_id = 'package-launch-1'",
    )
    .bind(digest('9'))
    .execute(&store.pool)
    .await;
    assert!(update.is_err());
    let delete = sqlx::query("DELETE FROM packages_v3 WHERE package_id = 'package-launch-1'")
        .execute(&store.pool)
        .await;
    assert!(delete.is_err());

    let late_package = sqlx::query(
        "INSERT INTO package_artifacts_v3 (
            package_id, artifact_id, position, role, display_name, media_type,
            size_bytes, sha256, storage_key
         ) VALUES ('package-launch-1', 'artifact-late', 1, 'attachment',
                   'entry', 'text/plain', 1, ?, 'test-object')",
    )
    .bind(digest('8'))
    .execute(&store.pool)
    .await;
    assert!(late_package.is_err());
    let child_update = sqlx::query(
        "UPDATE package_artifacts_v3 SET display_name = 'drifted'
         WHERE package_id = 'package-launch-1'",
    )
    .execute(&store.pool)
    .await;
    assert!(child_update.is_err());
    let child_delete =
        sqlx::query("DELETE FROM package_artifacts_v3 WHERE package_id = 'package-launch-1'")
            .execute(&store.pool)
            .await;
    assert!(child_delete.is_err());

    let mut request = waiting_request("immutable-request", "session-1");
    request.actions.push(FeedbackAction {
        id: "initial".to_owned(),
        instruction: "Initial action".to_owned(),
    });
    request.context_refs.push(ContextReference {
        label: "Initial context".to_owned(),
        uri: "rambledesk-context://initial".to_owned(),
    });
    request.request_artifacts.push(RequestArtifact {
        artifact_id: ArtifactId::new("artifact-request"),
        position: 0,
        display_name: "request entry".to_owned(),
        media_type: "text/plain".to_owned(),
        size_bytes: 1,
        sha256: digest('8'),
        storage_key: "test-object".to_owned(),
    });
    store
        .apply(FactMutation::FeedbackRequest(Box::new(
            FeedbackRequestCommit { request },
        )))
        .await
        .expect("feedback aggregate");
    let late_action = sqlx::query(
        "INSERT INTO feedback_request_actions_v3
         (request_id, action_id, position, instruction)
         VALUES ('immutable-request', 'late', 1, 'late action')",
    )
    .execute(&store.pool)
    .await;
    assert!(late_action.is_err());
    let late_submission = sqlx::query(
        "INSERT INTO submission_artifacts_v3 (
            submission_id, artifact_id, position, display_name, media_type,
            size_bytes, sha256, storage_key
         ) VALUES ('launch-1', 'submission-late', 1, 'late', 'text/plain', 1, ?, 'test-object')",
    )
    .bind(digest('8'))
    .execute(&store.pool)
    .await;
    assert!(late_submission.is_err());
}

#[tokio::test]
async fn consistency_report_accepts_imported_delivered_feedback_without_work() {
    let temp = TempDir::new().expect("tempdir");
    let store = open(&temp).await;
    sqlx::query(
        "INSERT INTO sessions_v3 (
            session_id, session_kind, title, lifecycle, launch_configuration_json,
            created_at, updated_at
         ) VALUES ('imported-session', 'connected', 'Imported', 'stopped', NULL, ?, ?)",
    )
    .bind(NOW)
    .bind(NOW)
    .execute(&store.pool)
    .await
    .expect("imported session");
    sqlx::query(
        "INSERT INTO feedback_requests_v3 (
            request_id, session_id, source_link_id, title, instructions, input_digest,
            resolution, response_package_id, cancel_reason, created_at, resolved_at, updated_at
         ) VALUES (
            'imported-request', 'imported-session', NULL, 'Imported request', 'Imported instructions', ?,
            'cancelled', NULL, 'Imported cancellation', ?, ?, ?
         )",
    )
    .bind(digest('7'))
    .bind(NOW)
    .bind(LATER)
    .bind(LATER)
    .execute(&store.pool)
    .await
    .expect("imported request");
    sqlx::query(
        "INSERT INTO feedback_deliveries_v3 (
            delivery_id, request_id, session_id, resolution, package_id, state,
            attempt_count, last_error_code, last_error_at, created_at, delivered_at
         ) VALUES (
            'imported-delivery', 'imported-request', 'imported-session', 'cancelled', NULL,
            'delivered', 0, NULL, NULL, ?, ?
         )",
    )
    .bind(LATER)
    .bind(LATER)
    .execute(&store.pool)
    .await
    .expect("imported delivery");
    let report = store
        .inspect_consistency()
        .await
        .expect("consistency report");
    assert!(report.is_consistent(), "{:?}", report.violations);
    assert_eq!(report.table_counts["feedback_deliveries_v3"], 1);
}
