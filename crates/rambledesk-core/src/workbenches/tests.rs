use super::*;

fn fixture() -> (WorkbenchSpec, FeedbackDraft) {
    let spec = WorkbenchSpec {
        kind: "document_review".into(),
        version: 1,
        data: WorkbenchData::DocumentReview(DocumentReviewData {
            title: "Speech".into(),
            source_version: "draft-1".into(),
            markdown: "你好😀 **world**\r\n\r\n## Next\n".into(),
        }),
    };
    let draft = FeedbackDraft {
        workbench_state: Some(WorkbenchState::DocumentReview {
            verdict: Some(ReviewVerdict::ChangesRequested),
            annotations: vec![ReviewAnnotation {
                id: "note-1".into(),
                start: Some(2),
                end: Some(3),
                quote: Some("😀".into()),
                kind: ReviewAnnotationKind::Suggestion,
                body: "Remove emoji".into(),
                replacement: Some(String::new()),
            }],
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
        |item| item.start = None,
        |item| item.start = Some(u32::MAX),
        |item| item.end = Some(4),
        |item| item.end = Some(100),
        |item| item.end = None,
        |item| item.quote = Some("wrong".into()),
        |item| item.quote = None,
        |item| {
            item.kind = ReviewAnnotationKind::Comment;
            item.replacement = None;
            item.body = " \n ".into();
        },
        |item| item.body = "a".repeat(4001),
        |item| item.body = "bad\0body".into(),
        |item| item.replacement = None,
        |item| item.replacement = Some("a".repeat(8001)),
        |item| item.replacement = Some("bad\0replacement".into()),
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
fn document_review_suggestions_allow_optional_reason_but_comments_require_body() {
    for body in ["", " \r\n\t "] {
        for replacement in ["", "修订😀"] {
            let (spec, mut draft) = fixture();
            let Some(WorkbenchState::DocumentReview { annotations, .. }) =
                &mut draft.workbench_state
            else {
                unreachable!()
            };
            annotations[0].body = body.into();
            annotations[0].replacement = Some(replacement.into());
            let package = prepare_feedback_submission(Some(&spec), Some(&draft), "")
                .unwrap()
                .unwrap();
            assert_eq!(package.input, spec);
            let Some(WorkbenchResult::DocumentReview(result)) = package.result else {
                unreachable!()
            };
            assert_eq!(result.annotations[0].body, body);
            assert_eq!(
                result.annotations[0].replacement.as_deref(),
                Some(replacement)
            );
            assert_eq!(result.annotations[0].quote.as_deref(), Some("😀"));

            let Some(WorkbenchState::DocumentReview { verdict, .. }) = &mut draft.workbench_state
            else {
                unreachable!()
            };
            *verdict = None;
            assert!(matches!(
                prepare_feedback_submission(Some(&spec), Some(&draft), ""),
                Err(WorkbenchSubmissionError::Incomplete)
            ));
        }

        let (spec, mut draft) = fixture();
        let Some(WorkbenchState::DocumentReview { annotations, .. }) = &mut draft.workbench_state
        else {
            unreachable!()
        };
        annotations[0].kind = ReviewAnnotationKind::Comment;
        annotations[0].replacement = None;
        annotations[0].body = body.into();
        assert!(matches!(
            prepare_feedback_submission(Some(&spec), Some(&draft), ""),
            Err(WorkbenchSubmissionError::Incomplete)
        ));
    }
}

#[test]
fn document_review_rejects_duplicate_ids_and_annotation_overflow() {
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
}

#[test]
fn document_review_validates_source_limits_without_legacy_actions() {
    let (mut spec, _) = fixture();
    assert!(workbench_actions(&spec).unwrap().is_empty());
    let WorkbenchData::DocumentReview(data) = &mut spec.data else {
        unreachable!()
    };
    data.markdown = "😀".repeat(120_000);
    assert!(validate_workbench(&spec).is_ok());
    let WorkbenchData::DocumentReview(data) = &mut spec.data else {
        unreachable!()
    };
    data.markdown.push('x');
    assert!(validate_workbench(&spec).is_err());
    for invalid in ["", " \r\n\t", "text\0more"] {
        let WorkbenchData::DocumentReview(data) = &mut spec.data else {
            unreachable!()
        };
        data.markdown = invalid.into();
        assert!(validate_workbench(&spec).is_err());
    }
}

#[test]
fn document_review_anchors_raw_markdown_syntax_and_line_endings_without_normalization() {
    let (spec, _) = fixture();
    let WorkbenchData::DocumentReview(data) = &spec.data else {
        unreachable!()
    };
    for quote in [
        "**world**",
        "😀 **world**\r\n\r\n## Next",
        "\r\n\r\n",
        "Next\n",
    ] {
        let (_, mut draft) = fixture();
        let Some(WorkbenchState::DocumentReview { annotations, .. }) = &mut draft.workbench_state
        else {
            unreachable!()
        };
        let start = data.markdown[..data.markdown.find(quote).unwrap()]
            .chars()
            .count();
        annotations[0].start = Some(start as u32);
        annotations[0].end = Some((start + quote.chars().count()) as u32);
        annotations[0].quote = Some(quote.into());
        let package = prepare_feedback_submission(Some(&spec), Some(&draft), "")
            .unwrap()
            .unwrap();
        assert_eq!(package.input, spec);
        let Some(WorkbenchResult::DocumentReview(result)) = package.result else {
            unreachable!()
        };
        assert_eq!(result.annotations[0].quote.as_deref(), Some(quote));
        assert_eq!(result.annotations[0].start, Some(start as u32));
        let Some(WorkbenchState::DocumentReview { annotations, .. }) = &mut draft.workbench_state
        else {
            unreachable!()
        };
        annotations[0].quote = Some(quote.replace("**", "").replace("\r\n", "\n").trim().into());
        assert!(workbench_result(&spec, draft.workbench_state.as_ref()).is_none());
    }
}

#[test]
fn document_review_whole_document_notes_require_all_three_null_anchor_fields() {
    let (spec, mut draft) = fixture();
    let Some(WorkbenchState::DocumentReview { annotations, .. }) = &mut draft.workbench_state
    else {
        unreachable!()
    };
    annotations[0].start = None;
    annotations[0].end = None;
    annotations[0].quote = None;
    annotations[0].kind = ReviewAnnotationKind::Comment;
    annotations[0].replacement = None;
    assert!(workbench_result(&spec, draft.workbench_state.as_ref()).is_some());
    for field in ["start", "end", "quote"] {
        let mut state = draft.workbench_state.clone().unwrap();
        let WorkbenchState::DocumentReview { annotations, .. } = &mut state else {
            unreachable!()
        };
        match field {
            "start" => annotations[0].start = Some(0),
            "end" => annotations[0].end = Some(0),
            _ => annotations[0].quote = Some("你好".into()),
        }
        assert!(workbench_result(&spec, Some(&state)).is_none());
    }
}

#[test]
fn document_review_rejects_retired_paragraph_contract_fields() {
    let (spec, draft) = fixture();
    let mut data = serde_value::to_value(&spec.data).unwrap();
    let serde_value::Value::Map(fields) = &mut data else {
        unreachable!()
    };
    fields.insert(
        serde_value::Value::String("paragraphs".into()),
        serde_value::Value::Seq(vec![]),
    );
    assert!(data.deserialize_into::<DocumentReviewData>().is_err());
    let mut state = serde_value::to_value(draft.workbench_state.unwrap()).unwrap();
    let serde_value::Value::Map(fields) = &mut state else {
        unreachable!()
    };
    fields.insert(
        serde_value::Value::String("paragraph_marks".into()),
        serde_value::Value::Seq(vec![]),
    );
    assert!(state.deserialize_into::<WorkbenchState>().is_err());
}
