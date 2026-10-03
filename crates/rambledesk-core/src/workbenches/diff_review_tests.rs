use super::*;
const DIFF: &str = "--- a/file\n+++ b/file\n@@ -10,3 +10,3 @@\n same\n-old\n+new\n tail\n\\ No newline at end of file\n@@ -20,0 +21,2 @@\n+first\n+second\n";
fn data() -> DiffReviewData {
    DiffReviewData {
        title: "Review".into(),
        source_version: "commit-v1".into(),
        files: vec![DiffReviewFile {
            id: "file".into(),
            old_path: "file".into(),
            new_path: "file".into(),
            diff: DIFF.into(),
        }],
    }
}
fn comment() -> DiffReviewComment {
    DiffReviewComment {
        id: "c-1".into(),
        anchor: DiffReviewAnchor {
            file_id: "file".into(),
            hunk_index: 0,
            side: DiffReviewSide::Old,
            start_line: Some(10),
            end_line: Some(12),
        },
        body: "Use a clearer name".into(),
    }
}
fn state(comments: Vec<DiffReviewComment>) -> WorkbenchState {
    WorkbenchState::DiffReview(DiffReviewState { comments })
}
#[test]
fn strict_parser_supports_context_sides_zero_count_crlf_and_no_newline_markers() {
    for diff in [
        DIFF.to_owned(),
        DIFF.replace('\n', "\r\n"),
        format!("{DIFF}\n"),
    ] {
        let hunks = parser::parse(&diff).unwrap();
        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[0].old, (10, 3));
        assert_eq!(hunks[1].new, (21, 2));
    }
    for invalid in [
        DIFF.replace("-10,3", "-10,4"),
        DIFF.replace("+10,3", "+10,2"),
        DIFF.replace("-10,3", "-0,3"),
        DIFF.replace("-20,0", "-11,0"),
        DIFF.replace(" tail", "tail"),
        format!("{DIFF}+extra\n"),
        format!("{DIFF}--- another-file\n"),
        "@@ -4294967295,1 +1 @@\n-x\n+y\n".into(),
        "@@@ -1,1 -1,1 +1,1 @@@\n--old\n++new\n".into(),
        "\\ No newline at end of file\n@@ -1 +1 @@\n-a\n+b\n".into(),
        "GIT binary patch\n".into(),
    ] {
        assert!(parser::parse(&invalid).is_none(), "{invalid}");
    }
}
#[test]
fn range_hunk_and_added_side_anchors_are_exact_and_foreign_or_gap_anchors_fail() {
    let data = data();
    validate(&data).unwrap();
    let c = comment();
    assert!(draft_valid(&data, Some(&state(vec![c.clone()]))));
    let mut whole = c.clone();
    whole.anchor.start_line = None;
    whole.anchor.end_line = None;
    assert!(result(&data, Some(&state(vec![whole]))).is_some());
    let mut added = c.clone();
    added.anchor.hunk_index = 1;
    added.anchor.side = DiffReviewSide::New;
    added.anchor.start_line = Some(21);
    added.anchor.end_line = Some(22);
    assert!(result(&data, Some(&state(vec![added.clone()]))).is_some());
    added.anchor.side = DiffReviewSide::Old;
    assert!(!draft_valid(&data, Some(&state(vec![added]))));
    let mut invalid = vec![];
    let mut a = c.clone();
    a.anchor.file_id = "foreign".into();
    invalid.push(a);
    let mut a = c.clone();
    a.anchor.hunk_index = 2;
    invalid.push(a);
    let mut a = c.clone();
    a.anchor.start_line = Some(9);
    invalid.push(a);
    let mut a = c.clone();
    a.anchor.end_line = Some(20);
    invalid.push(a);
    let mut a = c.clone();
    a.anchor.end_line = Some(9);
    invalid.push(a);
    let mut a = c.clone();
    a.anchor.start_line = None;
    invalid.push(a);
    let mut a = c.clone();
    a.body = "NUL\0".into();
    invalid.push(a);
    let mut a = c.clone();
    a.id = "UPPER".into();
    invalid.push(a);
    for comment in invalid {
        assert!(!draft_valid(&data, Some(&state(vec![comment.clone()]))));
        assert!(result(&data, Some(&state(vec![comment]))).is_none());
    }
    assert!(!draft_valid(&data, Some(&state(vec![c.clone(), c]))));
}
#[test]
fn notes_or_comments_publish_without_approval_and_unfinished_comments_cannot_hide_behind_notes() {
    let spec = WorkbenchSpec {
        kind: "diff_review".into(),
        version: 1,
        data: WorkbenchData::DiffReview(data()),
    };
    let draft = FeedbackDraft {
        workbench_state: Some(state(vec![comment()])),
    };
    let package = prepare_feedback_submission(Some(&spec), Some(&draft), "")
        .unwrap()
        .unwrap();
    assert!(complete(&data(), package.result.as_ref()));
    assert!(!definition().approval);
    let roundtrip: WorkbenchPackage = serde_value::to_value(&package)
        .unwrap()
        .deserialize_into()
        .unwrap();
    assert_eq!(roundtrip, package);
    let notes = prepare_feedback_submission(Some(&spec), None, "Overall notes")
        .unwrap()
        .unwrap();
    assert!(
        matches!(notes.result,Some(WorkbenchResult::DiffReview(result)) if result.comments.is_empty())
    );
    assert_eq!(
        prepare_feedback_submission(Some(&spec), None, "").unwrap_err(),
        WorkbenchSubmissionError::Empty
    );
    let mut comment = comment();
    comment.body = String::new();
    let state = state(vec![comment]);
    assert!(draft_valid(&data(), Some(&state)));
    let draft = FeedbackDraft {
        workbench_state: Some(state),
    };
    assert_eq!(
        prepare_feedback_submission(Some(&spec), Some(&draft), "Notes").unwrap_err(),
        WorkbenchSubmissionError::Incomplete
    );
    assert!(
        workbench_package(&spec, Some(&draft), false)
            .result
            .is_none()
    );
}
#[test]
fn schema_material_size_and_identity_limits_are_enforced() {
    validate_workbench(&describe().unwrap().example).unwrap();
    let mut invalid = data();
    invalid.files.push(invalid.files[0].clone());
    assert!(validate(&invalid).is_err());
    let mut invalid = data();
    invalid.files[0].diff = "x".repeat(120001);
    assert!(validate(&invalid).is_err());
    let mut invalid = data();
    invalid.files.clear();
    assert!(validate(&invalid).is_err());
    let mut invalid = data();
    invalid.files[0].old_path = " ".into();
    assert!(validate(&invalid).is_err());
    let mut invalid = data();
    invalid.files[0].id = "UPPER".into();
    assert!(validate(&invalid).is_err());
}
