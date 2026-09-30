use super::*;

fn fixture() -> (WorkbenchSpec, FeedbackDraft) {
    let spec = describe_workbench(&DescribeWorkbenchInput {
        kind: "web_review".into(),
        version: None,
    })
    .unwrap()
    .example;
    let draft = FeedbackDraft {
        workbench_state: Some(WorkbenchState::WebReview {
            annotations: vec![WebReviewAnnotation {
                id: "note-1".into(),
                page_url: "http://localhost:5173/settings?tab=profile".into(),
                viewport: WebReviewViewport {
                    width: 390,
                    height: 844,
                },
                element: WebReviewElement {
                    selector: "main > button:nth-of-type(2)".into(),
                    tag_name: "button".into(),
                    text: "保存😀".into(),
                    rect: WebReviewRect {
                        x: 20,
                        y: 1300,
                        width: 200,
                        height: 48,
                    },
                },
                body: "Keep this action visible".into(),
                screenshot_attachment_id: None,
            }],
        }),
    };
    (spec, draft)
}

#[test]
fn web_review_publishes_selection_time_context_without_verdict() {
    let (spec, draft) = fixture();
    let package = prepare_feedback_submission(Some(&spec), Some(&draft), "")
        .unwrap()
        .unwrap();
    let Some(WorkbenchResult::WebReview(result)) = package.result else {
        panic!("expected review")
    };
    assert_eq!(result.source_version, "draft-1");
    assert_eq!(
        result.annotations[0].page_url,
        "http://localhost:5173/settings?tab=profile"
    );
    assert_eq!(result.annotations[0].viewport.width, 390);
    assert_eq!(result.annotations[0].element.rect.y, 1300);
    assert_eq!(result.annotations[0].element.text, "保存😀");
    assert!(workbench_actions(&spec).unwrap().is_empty());
    assert!(!WorkbenchKind::WebReview.supports_approval());
    assert!(!WorkbenchKind::WebReview.uses_legacy_action_identity());
    assert!(
        workbench_package(&spec, Some(&draft), false)
            .result
            .is_none()
    );
}

#[test]
fn web_review_accepts_general_notes_and_rejects_an_empty_review() {
    let (spec, _) = fixture();
    for draft in [
        None,
        Some(FeedbackDraft {
            workbench_state: Some(WorkbenchState::WebReview {
                annotations: vec![],
            }),
        }),
        Some(FeedbackDraft {
            workbench_state: None,
        }),
    ] {
        assert_eq!(
            prepare_feedback_submission(Some(&spec), draft.as_ref(), " \n ").unwrap_err(),
            WorkbenchSubmissionError::Empty
        );
        let package = prepare_feedback_submission(
            Some(&spec),
            draft.as_ref(),
            "The whole page needs more contrast",
        )
        .unwrap()
        .unwrap();
        let Some(WorkbenchResult::WebReview(result)) = package.result else {
            panic!("expected review")
        };
        assert!(result.annotations.is_empty());
    }
}

#[test]
fn web_review_rejects_invalid_anchors_instead_of_publishing_only_notes() {
    let cases: &[fn(&mut WebReviewAnnotation)] = &[
        |item| item.id = "Invalid ID".into(),
        |item| item.body = " \n ".into(),
        |item| item.body = "a".repeat(4001),
        |item| item.body = "a\0b".into(),
        |item| item.page_url = "file:///C:/secret".into(),
        |item| item.page_url = "https://user:password@example.com/".into(),
        |item| item.page_url = "https://example.com/\n".into(),
        |item| item.viewport.width = 239,
        |item| item.viewport.height = 4321,
        |item| item.element.selector = "".into(),
        |item| item.element.selector = "a".repeat(2001),
        |item| item.element.tag_name = "BUTTON".into(),
        |item| item.element.text = "a".repeat(2001),
        |item| item.element.rect.x = 1_000_001,
        |item| item.element.rect.width = 0,
        |item| item.element.rect.height = 1_000_001,
        |item| item.screenshot_attachment_id = Some("../secret".into()),
    ];
    for (index, invalidate) in cases.iter().enumerate() {
        let (spec, mut draft) = fixture();
        let Some(WorkbenchState::WebReview { annotations }) = &mut draft.workbench_state else {
            unreachable!()
        };
        invalidate(&mut annotations[0]);
        assert_eq!(
            prepare_feedback_submission(Some(&spec), Some(&draft), "General notes").unwrap_err(),
            WorkbenchSubmissionError::Incomplete,
            "case {index}"
        );
    }
}

#[test]
fn web_review_rejects_duplicate_and_excessive_annotations() {
    for count in [2, 501] {
        let (spec, mut draft) = fixture();
        let Some(WorkbenchState::WebReview { annotations }) = &mut draft.workbench_state else {
            unreachable!()
        };
        let original = annotations[0].clone();
        *annotations = (0..count)
            .map(|index| WebReviewAnnotation {
                id: if count == 2 {
                    "duplicate".into()
                } else {
                    format!("note-{index}")
                },
                ..original.clone()
            })
            .collect();
        assert!(workbench_result(&spec, draft.workbench_state.as_ref()).is_none());
    }
}

#[test]
fn web_review_validates_initial_url_and_viewport() {
    let (spec, _) = fixture();
    for value in [
        "file:///index.html",
        "javascript:alert(1)",
        "https://user@example.com/",
        "https://example.com/a b",
        "https://example.com/\0",
        "https:example.com/",
        "https://example.com/\\path",
        "https://example.com/\u{feff}",
    ] {
        let mut invalid = spec.clone();
        let WorkbenchData::WebReview(data) = &mut invalid.data else {
            unreachable!()
        };
        data.url = value.into();
        assert!(validate_workbench(&invalid).is_err(), "accepted {value}");
    }
    let mut invalid = spec;
    let WorkbenchData::WebReview(data) = &mut invalid.data else {
        unreachable!()
    };
    data.viewport.width = 7681;
    assert!(validate_workbench(&invalid).is_err());
}
