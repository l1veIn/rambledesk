use super::*;

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
