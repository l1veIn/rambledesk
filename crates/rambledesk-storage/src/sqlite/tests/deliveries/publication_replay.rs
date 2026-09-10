use super::*;

#[tokio::test]
async fn lost_submit_response_replays_after_restart_without_republishing_or_reenqueuing() {
    let (workspace, store) = setup().await;
    let id = new_request(&store, Some("one")).await;
    let revision = prepare_draft(&store, &id).await;
    let completed = store
        .clone()
        .into_application()
        .submit_feedback(submission(&id, revision))
        .await
        .unwrap();
    // The backend committed and continuation finished, but the client never
    // received the submit response. Preserve the accepted delivery attempt.
    store
        .claim_delivery(&id, "accepted-attempt", NOW)
        .await
        .unwrap()
        .unwrap();
    store
        .finish_delivery(
            &id,
            "accepted-attempt",
            DeliveryState::Delivered,
            None,
            LATER,
        )
        .await
        .unwrap();
    let deliveries = store.list_session_deliveries("one").await.unwrap();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].state, DeliveryState::Delivered);
    let neighbor = approved(&store, "two").await;
    let neighbor_deliveries = store.list_session_deliveries("two").await.unwrap();
    let package = completed.feedback.as_ref().unwrap();
    let markdown = tokio::fs::read(&package.markdown_path).await.unwrap();
    let manifest = tokio::fs::read(&package.manifest_path).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
    assert_eq!(
        parsed["feedback_sha256"],
        hex::encode(sha2::Sha256::digest(&markdown))
    );
    let plans: i64 =
        sqlx::query_scalar("SELECT count(*) FROM submission_plans WHERE request_id=?1")
            .bind(&id)
            .fetch_one(&store.pool)
            .await
            .unwrap();
    store.close().await;

    // Independent pools reproduce separate clients after the original runtime
    // has gone away. Both retry stale revisions and different cooked content.
    let left = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let right = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let left_app = left.clone().into_application();
    let right_app = right.clone().into_application();
    let mut changed = submission(&id, 0);
    changed.cooked_markdown = Some("A retry must never replace the committed package.".into());
    changed.cooking_model = Some("retry/model".into());
    let (first, second) = tokio::join!(
        left_app.submit_feedback(submission(&id, 0)),
        right_app.submit_feedback(changed),
    );
    assert_eq!(first.unwrap(), completed);
    assert_eq!(second.unwrap(), completed);
    assert_eq!(
        left.list_session_deliveries("one").await.unwrap(),
        deliveries
    );
    assert_eq!(
        right.list_session_deliveries("two").await.unwrap(),
        neighbor_deliveries
    );
    assert_eq!(
        left.list_pending_deliveries()
            .await
            .unwrap()
            .into_iter()
            .map(|delivery| delivery.request_id)
            .collect::<Vec<_>>(),
        vec![neighbor]
    );
    assert_eq!(
        tokio::fs::read(&package.markdown_path).await.unwrap(),
        markdown
    );
    assert_eq!(
        tokio::fs::read(&package.manifest_path).await.unwrap(),
        manifest
    );
    let replayed_plans: i64 =
        sqlx::query_scalar("SELECT count(*) FROM submission_plans WHERE request_id=?1")
            .bind(&id)
            .fetch_one(&left.pool)
            .await
            .unwrap();
    assert_eq!(replayed_plans, plans);
    left.close().await;
    right.close().await;
}
