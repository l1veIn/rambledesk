use super::*;

#[tokio::test]
async fn turn_windows_include_long_work_between_prompts_and_keep_cursor_pages_bounded() {
    let (_workspace, store) = setup().await;
    let mut sequence = 0;
    for turn in 0..25 {
        let count = if turn == 24 { 300 } else { 4 };
        for offset in 0..count {
            sequence += 1;
            let mut row = activity(&format!("one-{sequence}"), "one");
            row.turn_id = Some(format!("turn-{turn}"));
            row.kind = if offset == 0 {
                SessionActivityKind::UserMessage
            } else {
                SessionActivityKind::AgentMessage
            };
            store.append_activity(row).await.unwrap();
        }
    }
    let recent = store
        .list_recent_session_activity("one", 100)
        .await
        .unwrap();
    assert!(
        recent
            .iter()
            .all(|row| row.turn_id.as_deref() == Some("turn-24"))
    );
    let history = store
        .list_session_turn_activity_before("one", sequence + 1, 20, 1_000)
        .await
        .unwrap();
    assert_eq!(history.len(), 19 * 4 + 300);
    assert_eq!(
        history
            .iter()
            .filter(|row| row.kind == SessionActivityKind::UserMessage)
            .count(),
        20
    );
    assert_eq!(history[0].turn_id.as_deref(), Some("turn-5"));
    let bounded = store
        .list_session_turn_activity_before("one", sequence + 1, 20, 150)
        .await
        .unwrap();
    assert_eq!(bounded.len(), 150);
    let mut all = bounded.clone();
    while all[0].sequence > 1 {
        let earlier = store
            .list_session_turn_activity_before("one", all[0].sequence, 20, 150)
            .await
            .unwrap();
        assert!(!earlier.is_empty());
        assert!(earlier.len() <= 150);
        all.splice(0..0, earlier);
    }
    assert_eq!(
        all.iter().map(|row| row.sequence).collect::<Vec<_>>(),
        (1..=sequence).collect::<Vec<_>>()
    );
    let plan = sqlx::query("EXPLAIN QUERY PLAN SELECT sequence FROM session_activity WHERE session_id = ?1 AND sequence < ?2 AND kind = 'user_message' ORDER BY sequence DESC LIMIT 1 OFFSET 19")
        .bind("one").bind(sequence as i64 + 1).fetch_all(&store.pool).await.unwrap();
    assert!(plan.iter().any(|row| {
        row.get::<String, _>("detail")
            .contains("idx_session_activity_user_turn")
    }));
}

#[tokio::test]
async fn turn_history_limits_payload_but_an_oversize_row_always_advances_the_cursor() {
    let (_workspace, store) = setup().await;
    for index in 1..=3 {
        let mut row = activity(&format!("large-{index}"), "one");
        row.text = "x".repeat(900_000);
        store.append_activity(row).await.unwrap();
    }
    let bounded = store
        .list_session_turn_activity_before("one", 4, 20, 1000)
        .await
        .unwrap();
    assert_eq!(
        bounded.iter().map(|row| row.sequence).collect::<Vec<_>>(),
        vec![2, 3]
    );
    let mut oversized = activity("oversized", "one");
    oversized.text = "x".repeat(2_200_000);
    store.append_activity(oversized).await.unwrap();
    let one = store
        .list_session_turn_activity_before("one", 5, 20, 1000)
        .await
        .unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].sequence, 4);
    assert_eq!(
        store
            .list_session_turn_activity_before("missing", 5, 20, 1000)
            .await
            .unwrap_err(),
        SessionRepositoryError::SessionNotFound
    );
    for count in [0, 51] {
        assert_eq!(
            store
                .list_session_turn_activity_before("one", 5, count, 1000)
                .await
                .unwrap_err(),
            SessionRepositoryError::InvalidInput
        );
    }
}

#[tokio::test]
async fn older_pages_are_exclusive_ordered_and_stable_while_new_rows_arrive() {
    let (_workspace, store) = setup().await;
    for number in 1..=7 {
        store
            .append_activity(activity(&format!("one-{number}"), "one"))
            .await
            .unwrap();
        store
            .append_activity(activity(&format!("two-{number}"), "two"))
            .await
            .unwrap();
    }
    let latest = store.list_recent_session_activity("one", 3).await.unwrap();
    assert_eq!(
        latest.iter().map(|row| row.sequence).collect::<Vec<_>>(),
        vec![5, 6, 7]
    );
    store
        .append_activity(activity("one-8", "one"))
        .await
        .unwrap();
    store
        .update_activity_text("one-4", "one", "Final text")
        .await
        .unwrap();
    let older = store
        .list_session_activity_before("one", latest[0].sequence, 3)
        .await
        .unwrap();
    assert_eq!(
        older.iter().map(|row| row.sequence).collect::<Vec<_>>(),
        vec![2, 3, 4]
    );
    assert!(older.iter().all(|row| row.session_id == "one"));
    assert_eq!(older.last().unwrap().text, "Final text");
    let first = store
        .list_session_activity_before("one", older[0].sequence, 3)
        .await
        .unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].sequence, 1);
    assert!(
        store
            .list_session_activity_before("one", 1, 3)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store
            .list_session_activity_before("missing", 5, 3)
            .await
            .unwrap_err(),
        SessionRepositoryError::SessionNotFound
    );
    assert_eq!(
        store
            .list_session_activity_before("one", 0, 3)
            .await
            .unwrap_err(),
        SessionRepositoryError::InvalidInput
    );
    assert_eq!(
        store
            .list_session_activity_before("one", u64::MAX, 3)
            .await
            .unwrap_err(),
        SessionRepositoryError::InvalidInput
    );
    assert_eq!(
        store
            .list_session_activity_before("one", 5, 1001)
            .await
            .unwrap_err(),
        SessionRepositoryError::InvalidInput
    );
}
