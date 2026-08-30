use super::{
    AcpSessionLinkObservation, AgentObservation, CancelFeedbackRequest, DraftId, DraftMutation,
    RambleIntent, ResolveFeedbackRequest, SaveDraft, WorkbenchQuery,
    test_fixtures::{create_request, harness, launch_input},
};

#[tokio::test]
async fn workbench_projection_has_a_stable_domain_order() {
    let (core, _) = harness();
    let first = core
        .launch(launch_input("order-launch-a", "First"))
        .await
        .expect("first launch");
    let second = core
        .launch(launch_input("order-launch-b", "Second"))
        .await
        .expect("second launch");
    for (session_id, acp_session_id, digest_byte) in [
        (first.session_id.clone(), "order-acp-a", 'a'),
        (second.session_id.clone(), "order-acp-b", 'b'),
    ] {
        core.record_agent_observation(AgentObservation::AcpSessionLinked(
            AcpSessionLinkObservation {
                session_id,
                agent_profile_id: "codex".to_owned(),
                launch_profile_id: "local".to_owned(),
                acp_session_id: acp_session_id.to_owned(),
                capabilities_json: "{}".to_owned(),
                session_toolset_digest: format!("sha256:{}", digest_byte.to_string().repeat(64)),
            },
        ))
        .await
        .expect("link");
    }
    create_request(&core, first.session_id.clone(), "order-wait-a").await;
    create_request(&core, second.session_id.clone(), "order-wait-b").await;
    let cancel_a = create_request(&core, first.session_id.clone(), "order-cancel-a").await;
    let cancel_b = create_request(&core, second.session_id.clone(), "order-cancel-b").await;
    for request in [cancel_a, cancel_b] {
        core.resolve_feedback(ResolveFeedbackRequest::Cancel(CancelFeedbackRequest {
            request_id: request.request_id,
            reason: "Order test".to_owned(),
        }))
        .await
        .expect("cancel");
    }
    for (draft_id, session_id) in [
        ("order-draft-a", first.session_id),
        ("order-draft-b", second.session_id),
    ] {
        core.mutate_draft(DraftMutation::Save(SaveDraft {
            draft_id: DraftId::from(draft_id),
            intent: RambleIntent::Steering,
            session_id: Some(session_id),
            request_id: None,
            launch_configuration: None,
            document_json: "{}".to_owned(),
            body_markdown: "Draft".to_owned(),
            expected_revision: 0,
        }))
        .await
        .expect("draft");
    }

    let first_read = core
        .read_workbench(WorkbenchQuery { session_id: None })
        .await
        .expect("workbench");
    let second_read = core
        .read_workbench(WorkbenchQuery { session_id: None })
        .await
        .expect("stable workbench");
    assert_eq!(first_read, second_read);

    let mut sessions = first_read.sessions.clone();
    sessions.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| right.session_id.cmp(&left.session_id))
    });
    assert_eq!(first_read.sessions, sessions);
    let mut links = first_read.current_acp_links.clone();
    links.sort_by(|left, right| {
        right
            .last_used_at
            .cmp(&left.last_used_at)
            .then_with(|| right.link_id.cmp(&left.link_id))
    });
    assert_eq!(first_read.current_acp_links, links);
    let mut waiting = first_read.waiting_feedback.clone();
    waiting.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.request_id.cmp(&right.request_id))
    });
    assert_eq!(first_read.waiting_feedback, waiting);
    let mut drafts = first_read.drafts.clone();
    drafts.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| right.draft_id.cmp(&left.draft_id))
    });
    assert_eq!(first_read.drafts, drafts);
    let mut deliveries = first_read.pending_deliveries.clone();
    deliveries.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.delivery_id.cmp(&right.delivery_id))
    });
    assert_eq!(first_read.pending_deliveries, deliveries);
    let mut work = first_read.pending_agent_work.clone();
    work.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.work_id.cmp(&right.work_id))
    });
    assert_eq!(first_read.pending_agent_work, work);
}
