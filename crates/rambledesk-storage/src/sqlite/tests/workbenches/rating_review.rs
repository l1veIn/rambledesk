use super::*;
use rambledesk_core::{RatingReviewState, WorkbenchResult, WorkbenchState};

fn document(score: Option<u32>, note: &str) -> String {
    json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},
        "workbenchState":WorkbenchState::RatingReview(RatingReviewState {score,note:note.into()})})
    .to_string()
}

#[tokio::test]
async fn rating_fixture_uses_real_discovery_save_recovery_cas_and_publication() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let catalog = list_workbenches(&ListWorkbenchesInput {
        offset: 0,
        limit: Some(20),
    })
    .unwrap();
    assert!(
        catalog
            .workbenches
            .iter()
            .any(|entry| entry.kind == "rating_review")
    );
    let description = describe_workbench(&DescribeWorkbenchInput {
        kind: "rating_review".into(),
        version: Some(1),
    })
    .unwrap();
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.actions.clear();
    input.workbench = Some(description.example);
    let created = app.request_feedback(input).await.unwrap();
    let incomplete = document(None, "Needs a more focused explanation.");
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: incomplete.clone(),
            body_markdown: "Body text must not bypass the required score.".into(),
            expected_revision: 0,
        })
        .await
        .unwrap();
    assert!(
        app.submit_feedback(SubmitFeedbackInput {
            request_id: created.request_id.clone(),
            expected_revision: saved.saved_revision,
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None
        })
        .await
        .is_err()
    );
    store.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    let recovered = app
        .get_feedback_workspace(created.request_id.clone())
        .await
        .unwrap();
    assert_eq!(
        recovered.draft.document_json.as_deref(),
        Some(incomplete.as_str())
    );
    assert!(
        app.save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: document(Some(4), "stale"),
            body_markdown: "stale".into(),
            expected_revision: 0
        })
        .await
        .is_err()
    );
    let ready = document(Some(4), "Clear and ready to use.");
    let saved = app
        .save_feedback_draft(SaveDraftInput {
            request_id: created.request_id.clone(),
            document_json: ready.clone(),
            body_markdown: String::new(),
            expected_revision: recovered.draft.saved_revision,
        })
        .await
        .unwrap();
    assert!(
        app.submit_feedback(SubmitFeedbackInput {
            request_id: created.request_id.clone(),
            expected_revision: 0,
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None
        })
        .await
        .is_err()
    );
    let result = app
        .submit_feedback(SubmitFeedbackInput {
            request_id: created.request_id.clone(),
            expected_revision: saved.saved_revision,
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None,
        })
        .await
        .unwrap();
    let package = app.read_feedback_package(&result).await.unwrap().unwrap();
    assert!(matches!(package.manifest.workbench.unwrap().result,
        Some(WorkbenchResult::RatingReview(result)) if result.score == 4 && result.note == "Clear and ready to use."));
    let frozen = app
        .get_feedback_workspace(created.request_id.clone())
        .await
        .unwrap();
    assert_eq!(frozen.request.status, FeedbackStatus::Completed);
    assert_eq!(frozen.draft.document_json.as_deref(), Some(ready.as_str()));
    let future: WorkbenchSpec = serde_json::from_value(json!({"type":"rating_review","version":99,
        "data":{"title":"Future","material":"Keep all fields","future":{"value":true}}}))
    .unwrap();
    assert!(matches!(future.data, WorkbenchData::Unknown(_)));
    assert!(rambledesk_core::validate_workbench(&future).is_err());
}

#[tokio::test]
async fn rating_fixture_strict_state_cannot_publish_malformed_or_invalid_values_as_notes_only() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let app = store.clone().into_application();
    for state in [
        json!({"type":"rating_review","score":7,"note":"bad range"}),
        json!({"type":"rating_review","score":4,"note":"valid","future":"must preserve"}),
        json!({"type":"rating_review","score":4,"note":"x".repeat(4001)}),
    ] {
        let mut input = workspace.request(Uuid::now_v7().to_string());
        input.actions.clear();
        input.workbench = Some(
            describe_workbench(&DescribeWorkbenchInput {
                kind: "rating_review".into(),
                version: None,
            })
            .unwrap()
            .example,
        );
        let created = app.request_feedback(input).await.unwrap();
        let saved = app.save_feedback_draft(SaveDraftInput {request_id:created.request_id.clone(),
            document_json:json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},"workbenchState":state}).to_string(),
            body_markdown:"These notes cannot authorize an invalid score.".into(),expected_revision:0}).await;
        assert!(saved.is_err());
        assert!(
            app.submit_feedback(SubmitFeedbackInput {
                request_id: created.request_id.clone(),
                expected_revision: 0,
                cooked_markdown: None,
                cooking_model: None,
                uncooked_markdown: None
            })
            .await
            .is_err()
        );
        assert!(
            app.get_feedback_workspace(created.request_id)
                .await
                .unwrap()
                .feedback
                .is_none()
        );
    }
}
