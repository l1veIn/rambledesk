use super::*;

fn fixture() -> (WorkbenchSpec, FeedbackDraft) {
    let spec = WorkbenchSpec {
        kind: "document_review".into(),
        version: 1,
        data: WorkbenchData::DocumentReview(DocumentReviewData {
            title: "Speech".into(),
            source_version: "draft-1".into(),
            paragraphs: vec![ReviewParagraph {
                id: "opening".into(),
                label: None,
                text: "你好😀 world".into(),
            }],
        }),
    };
    let draft = FeedbackDraft {
        workbench_state: Some(WorkbenchState::DocumentReview {
            verdict: Some(ReviewVerdict::ChangesRequested),
            annotations: vec![ReviewAnnotation {
                id: "note-1".into(),
                paragraph_id: "opening".into(),
                start: Some(2),
                end: Some(3),
                quote: Some("😀".into()),
                kind: ReviewAnnotationKind::Suggestion,
                body: "Remove emoji".into(),
                replacement: Some(String::new()),
            }],
            paragraph_marks: vec![],
        }),
    };
    (spec, draft)
}

#[test]
fn domain_review_preserves_source_anchor_and_allows_proposed_deletion() {
    let (spec, draft) = fixture();
    let package = prepare_feedback_submission(Some(&spec), Some(&draft), "")
        .unwrap()
        .unwrap();
    let Some(WorkbenchResult::DocumentReview(result)) = package.result else {
        panic!("expected review")
    };
    assert_eq!(result.source_version, "draft-1");
    assert_eq!(result.annotations[0].quote.as_deref(), Some("😀"));
    assert_eq!(result.annotations[0].replacement.as_deref(), Some(""));
    assert!(
        workbench_package(&spec, Some(&draft), false)
            .result
            .is_none()
    );
}

#[test]
fn domain_review_rejects_forged_anchor_and_notes_cannot_replace_verdict() {
    let (spec, mut draft) = fixture();
    let Some(WorkbenchState::DocumentReview { verdict, .. }) = &mut draft.workbench_state else {
        unreachable!()
    };
    *verdict = None;
    assert!(matches!(
        prepare_feedback_submission(Some(&spec), Some(&draft), "Go ahead"),
        Err(WorkbenchSubmissionError::Incomplete)
    ));
    let Some(WorkbenchState::DocumentReview {
        verdict,
        annotations,
        ..
    }) = &mut draft.workbench_state
    else {
        unreachable!()
    };
    *verdict = Some(ReviewVerdict::Ready);
    annotations[0].end = Some(4);
    assert!(matches!(
        prepare_feedback_submission(Some(&spec), Some(&draft), "Go ahead"),
        Err(WorkbenchSubmissionError::Incomplete)
    ));
}

#[test]
fn document_review_rejects_forged_or_incomplete_annotations() {
    let invalid_annotations: &[fn(&mut ReviewAnnotation)] = &[
        |item| item.start = Some(3),
        |item| item.end = Some(4),
        |item| item.end = Some(100),
        |item| item.end = None,
        |item| item.quote = Some("wrong".into()),
        |item| item.quote = None,
        |item| item.body = " \n ".into(),
        |item| item.body = "a".repeat(4001),
        |item| item.replacement = None,
        |item| item.replacement = Some("a".repeat(8001)),
        |item| item.paragraph_id = "missing".into(),
        |item| item.id = "Invalid ID".into(),
        |item| item.kind = ReviewAnnotationKind::Comment,
    ];
    for (index, invalidate) in invalid_annotations.iter().enumerate() {
        let (spec, mut draft) = fixture();
        let Some(WorkbenchState::DocumentReview { annotations, .. }) = &mut draft.workbench_state
        else {
            unreachable!()
        };
        invalidate(&mut annotations[0]);
        assert!(
            workbench_result(&spec, draft.workbench_state.as_ref()).is_none(),
            "accepted invalid annotation case {index}"
        );
    }
}

#[test]
fn document_review_rejects_duplicate_or_unknown_items_and_annotation_overflow() {
    let (spec, mut draft) = fixture();
    let Some(WorkbenchState::DocumentReview { annotations, .. }) = &mut draft.workbench_state
    else {
        unreachable!()
    };
    annotations.push(annotations[0].clone());
    assert!(workbench_result(&spec, draft.workbench_state.as_ref()).is_none());

    let (spec, mut draft) = fixture();
    let Some(WorkbenchState::DocumentReview { annotations, .. }) = &mut draft.workbench_state
    else {
        unreachable!()
    };
    let original = annotations[0].clone();
    *annotations = (0..501)
        .map(|index| ReviewAnnotation {
            id: format!("note-{index}"),
            ..original.clone()
        })
        .collect();
    assert!(workbench_result(&spec, draft.workbench_state.as_ref()).is_none());

    for paragraph_ids in [vec!["opening", "opening"], vec!["missing"]] {
        let (spec, mut draft) = fixture();
        let Some(WorkbenchState::DocumentReview {
            paragraph_marks, ..
        }) = &mut draft.workbench_state
        else {
            unreachable!()
        };
        *paragraph_marks = paragraph_ids
            .into_iter()
            .map(|paragraph_id| ParagraphMark {
                paragraph_id: paragraph_id.into(),
                decision: ParagraphDecision::Revise,
            })
            .collect();
        assert!(workbench_result(&spec, draft.workbench_state.as_ref()).is_none());
    }
}

#[test]
fn document_review_validates_source_limits_without_legacy_actions() {
    let (mut spec, _) = fixture();
    assert!(workbench_actions(&spec).unwrap().is_empty());
    let WorkbenchData::DocumentReview(data) = &mut spec.data else {
        unreachable!()
    };
    data.paragraphs = (0..15)
        .map(|index| ReviewParagraph {
            id: format!("p-{index}"),
            label: None,
            text: "😀".repeat(8000),
        })
        .collect();
    assert!(validate_workbench(&spec).is_ok());
    let WorkbenchData::DocumentReview(data) = &mut spec.data else {
        unreachable!()
    };
    data.paragraphs.push(ReviewParagraph {
        id: "extra".into(),
        label: None,
        text: "x".into(),
    });
    assert!(validate_workbench(&spec).is_err());
}
