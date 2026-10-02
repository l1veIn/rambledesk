use super::*;

#[tokio::test]
async fn stopped_and_naturally_exited_sessions_restart_with_retained_history() {
    let directory = tempfile::tempdir().unwrap();
    let manager = TerminalSessionManager::default();
    let open = || {
        manager.open(
            "request".into(),
            directory.path().to_string_lossy().into(),
            None,
            80,
            24,
        )
    };
    let first = open().await.unwrap();
    initial_cursor(&manager, "request", &first.session_id).await;
    manager
        .write(WriteTerminalSessionInput {
            request_id: "request".into(),
            session_id: first.session_id.clone(),
            data: shell_command(directory.path()),
        })
        .await
        .unwrap();
    until(&manager, "request", &first.session_id, "终端输出").await;
    let stopped = manager
        .stop(TerminalSessionInput {
            request_id: "request".into(),
            session_id: first.session_id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(stopped.status, TerminalSessionStatus::Stopped);
    let second = open().await.unwrap();
    assert_ne!(second.session_id, first.session_id);
    assert_eq!(second.status, TerminalSessionStatus::Running);
    assert_eq!(
        manager
            .read(&read("request", &first.session_id, None))
            .unwrap(),
        stopped,
        "reopening must preserve the old session's final transcript"
    );
    assert_eq!(open().await.unwrap().session_id, second.session_id);
    assert!(
        manager
            .write(WriteTerminalSessionInput {
                request_id: "request".into(),
                session_id: first.session_id.clone(),
                data: "old-session-input".into(),
            })
            .await
            .is_err()
    );
    initial_cursor(&manager, "request", &second.session_id).await;
    manager
        .write(WriteTerminalSessionInput {
            request_id: "request".into(),
            session_id: second.session_id.clone(),
            data: "exit 9\r".into(),
        })
        .await
        .unwrap();
    let exited = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = manager
                .read(&read("request", &second.session_id, None))
                .unwrap();
            if snapshot.status == TerminalSessionStatus::Exited {
                break snapshot;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(exited.exit_code, Some(9));
    let (third, same) = tokio::join!(open(), open());
    let third = third.unwrap();
    assert_eq!(third.session_id, same.unwrap().session_id);
    assert_ne!(third.session_id, second.session_id);
    assert_eq!(
        manager
            .read(&read("request", &second.session_id, None))
            .unwrap(),
        exited
    );
    assert!(
        manager
            .read(&read("other-request", &second.session_id, None))
            .is_err()
    );
    manager.stop_request("request");
    assert!(open().await.is_err());
    for id in [&first.session_id, &second.session_id, &third.session_id] {
        let final_snapshot = manager
            .stop(TerminalSessionInput {
                request_id: "request".into(),
                session_id: id.clone(),
            })
            .await
            .unwrap();
        assert_ne!(final_snapshot.status, TerminalSessionStatus::Running);
        assert!(final_snapshot.exit_code.is_some());
        assert!(!final_snapshot.truncated);
    }
}

#[tokio::test]
async fn request_session_limit_preserves_all_sixteen_finished_sessions() {
    let directory = tempfile::tempdir().unwrap();
    let manager = TerminalSessionManager::default();
    let mut history = Vec::new();
    for _ in 0..MAX_REQUEST_SESSIONS {
        let opened = manager
            .open(
                "request".into(),
                directory.path().to_string_lossy().into(),
                None,
                80,
                24,
            )
            .await
            .unwrap();
        let stopped = manager
            .stop(TerminalSessionInput {
                request_id: "request".into(),
                session_id: opened.session_id,
            })
            .await
            .unwrap();
        assert!(!stopped.truncated);
        assert!(
            history
                .iter()
                .all(|old: &TerminalSessionSnapshot| old.session_id != stopped.session_id)
        );
        history.push(stopped);
    }
    let error = manager
        .open(
            "request".into(),
            directory.path().to_string_lossy().into(),
            None,
            80,
            24,
        )
        .await
        .unwrap_err();
    assert!(error.message().contains("16 terminal sessions"));
    for previous in history {
        assert_eq!(
            manager
                .read(&read("request", &previous.session_id, None))
                .unwrap(),
            previous
        );
    }
}

#[tokio::test]
async fn finished_history_remains_reclaimable_after_request_tombstone_rollover() {
    let directory = tempfile::tempdir().unwrap();
    let manager = TerminalSessionManager::default();
    let opened = manager
        .open(
            "old-request".into(),
            directory.path().to_string_lossy().into(),
            None,
            80,
            24,
        )
        .await
        .unwrap();
    manager
        .stop(TerminalSessionInput {
            request_id: "old-request".into(),
            session_id: opened.session_id.clone(),
        })
        .await
        .unwrap();
    manager.stop_request("old-request");
    // Request completions with no PTY must not remove the retained entry's
    // reclamation marker. This creates one real PTY, rather than 4096 processes.
    for index in 0..MAX_CLOSED_REQUESTS {
        manager.stop_request(&format!("no-terminal-{index}"));
    }
    assert!(
        manager
            .open(
                "old-request".into(),
                directory.path().to_string_lossy().into(),
                None,
                80,
                24,
            )
            .await
            .is_err(),
        "a retained closed request must remain closed after tombstone rollover"
    );
    let mut registry = manager.inner.registry.lock().unwrap();
    assert_eq!(registry.closed.len(), MAX_CLOSED_REQUESTS);
    assert!(!registry.closed.iter().any(|id| id == "old-request"));
    registry.evict_finished().unwrap();
    assert!(!registry.sessions.contains_key(&opened.session_id));
    assert!(registry.order.is_empty());
}
