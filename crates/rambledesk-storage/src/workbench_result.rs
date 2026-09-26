use rambledesk_core::{FeedbackDraft, WorkbenchPackage, WorkbenchSpec, WorkbenchSubmissionError};

/// Storage owns this envelope codec; the core policy receives parsed domain data.
fn parse_feedback_draft(document: Option<&str>) -> Option<FeedbackDraft> {
    let value: serde_json::Value = serde_json::from_str(document?).ok()?;
    if value["schemaVersion"] != 2 || value["doc"]["type"] != "doc" {
        return None;
    }
    Some(FeedbackDraft {
        workbench_state: value
            .get("workbenchState")
            .cloned()
            .and_then(|state| serde_json::from_value(state).ok()),
    })
}

pub(crate) fn workbench_package(
    spec: &WorkbenchSpec,
    document: Option<&str>,
    submitted: bool,
) -> WorkbenchPackage {
    rambledesk_core::workbench_package(spec, parse_feedback_draft(document).as_ref(), submitted)
}

pub(crate) fn prepare_feedback_submission(
    spec: Option<&WorkbenchSpec>,
    document: Option<&str>,
    body: &str,
) -> Result<Option<WorkbenchPackage>, WorkbenchSubmissionError> {
    rambledesk_core::prepare_feedback_submission(
        spec,
        parse_feedback_draft(document).as_ref(),
        body,
    )
}

#[cfg(test)]
mod tests {
    use super::{prepare_feedback_submission, workbench_package};
    use rambledesk_core::*;
    use serde_json::{Value, json};

    fn spec() -> WorkbenchSpec {
        serde_json::from_value(json!({"type":"document_review","version":1,"data":{
            "title":"Speech", "source_version":"draft-a",
            "paragraphs":[{"id":"opening","text":"你好😀 world"}]
        }}))
        .unwrap()
    }

    fn state() -> Value {
        json!({"type":"document_review", "verdict":"changes_requested", "annotations":[{
            "id":"note-1", "paragraph_id":"opening", "start":2,"end":3,"quote":"😀",
            "kind":"suggestion", "body":"Remove this emoji", "replacement":"", "status":"open"
        }],"paragraph_marks":[{"paragraph_id":"opening","decision":"revise"}]})
    }

    fn result(value: Value) -> Option<WorkbenchResult> {
        let state = serde_json::from_value::<WorkbenchState>(value).ok()?;
        workbench_result(&spec(), Some(&state))
    }

    #[test]
    fn document_review_preserves_scalar_anchors_and_explicit_deletion() {
        let value = serde_json::to_value(result(state()).unwrap()).unwrap();
        assert_eq!(value["source_version"], "draft-a");
        assert_eq!(value["annotations"][0]["quote"], "😀");
        assert_eq!(value["annotations"][0]["replacement"], "");
        assert_eq!(value["annotations"][0]["start"], 2);
        assert!(value.get("cancelled").is_none());
        let mut whole = state();
        for field in ["start", "end", "quote", "replacement"] {
            whole["annotations"][0][field] = Value::Null;
        }
        whole["annotations"][0]["kind"] = json!("comment");
        assert!(result(whole).is_some());
    }

    #[test]
    fn document_review_rejects_forged_or_incomplete_annotations() {
        for (field, value) in [
            ("start", json!(-1)),
            ("start", json!(3)),
            ("end", json!(4)),
            ("end", json!(100)),
            ("end", Value::Null),
            ("quote", json!("wrong")),
            ("quote", Value::Null),
            ("body", json!(" \n ")),
            ("body", json!("a".repeat(4001))),
            ("replacement", Value::Null),
            ("replacement", json!("a".repeat(8001))),
            ("paragraph_id", json!("missing")),
            ("id", json!("Invalid ID")),
            ("kind", json!("comment")),
        ] {
            let mut invalid = state();
            invalid["annotations"][0][field] = value;
            assert!(result(invalid).is_none(), "accepted invalid {field}");
        }
        let mut duplicate = state();
        let annotation = duplicate["annotations"][0].clone();
        duplicate["annotations"]
            .as_array_mut()
            .unwrap()
            .push(annotation);
        assert!(result(duplicate).is_none());
        let mut duplicate = state();
        let mark = duplicate["paragraph_marks"][0].clone();
        duplicate["paragraph_marks"]
            .as_array_mut()
            .unwrap()
            .push(mark);
        assert!(result(duplicate).is_none());
        let mut missing = state();
        missing["paragraph_marks"][0]["paragraph_id"] = json!("missing");
        assert!(result(missing).is_none());
        let mut many = state();
        many["annotations"] = json!(
            (0..501)
                .map(|index| {
                    let mut annotation = state()["annotations"][0].clone();
                    annotation["id"] = json!(format!("note-{index}"));
                    annotation
                })
                .collect::<Vec<_>>()
        );
        assert!(result(many).is_none());
    }

    #[test]
    fn document_review_requires_verdict_and_never_uses_notes_as_a_verdict() {
        let mut incomplete = state();
        incomplete["verdict"] = Value::Null;
        let document = json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},"workbenchState":incomplete}).to_string();
        for notes in ["", "Please go ahead"] {
            assert!(matches!(
                prepare_feedback_submission(Some(&spec()), Some(&document), notes),
                Err(WorkbenchSubmissionError::Incomplete)
            ));
        }
        let document =
            json!({"schemaVersion":2,"doc":{"type":"doc","content":[]},"workbenchState":{
                "type":"document_review","verdict":"ready","annotations":[],"paragraph_marks":[]
            }})
            .to_string();
        assert!(prepare_feedback_submission(Some(&spec()), Some(&document), "").is_ok());
        assert!(
            workbench_package(&spec(), Some(&document), false)
                .result
                .is_none()
        );
    }

    #[test]
    fn document_review_validates_source_limits_without_legacy_actions() {
        let mut spec = spec();
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

    #[test]
    fn unknown_contracts_remain_readable_and_cannot_publish_free_text() {
        for wire in [
            json!({"type":"future_canvas","version":1,"data":{"shapes":[]}}),
            json!({"type":"ramble","version":2,"data":{"actions":[{"id":"a","instruction":"Review"}]}}),
            json!({"type":"questions","version":1,"data":{"future_field":[]}}),
            json!({"type":"ramble","version":2,"data":{"actions":[{"id":"a","instruction":"Review","future":{"nested":[1,2]}}]}}),
            json!({"type":"ramble","version":1,"data":{"actions":[{"id":"a","instruction":"Review","future":{"nested":[1,2]}}]}}),
            json!({"type":"questions","version":2,"data":{"questions":[{"id":"q","prompt":"Question","options":[{"value":"a","label":"A"},{"value":"b","label":"B"}]}]}}),
            json!({"type":"questions","version":1,"data":{"questions":[{"id":"q","prompt":"Question","options":[{"value":"a","label":"A"}]}]}}),
        ] {
            let spec: WorkbenchSpec = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(&spec).unwrap(), wire);
            assert!(matches!(
                prepare_feedback_submission(Some(&spec), None, "Free text"),
                Err(WorkbenchSubmissionError::Unsupported)
            ));
            let mut package = wire;
            // This resembles a known result with a missing optional field. A
            // future contract must preserve it rather than injecting null.
            package["result"] = json!({"status":"answered"});
            let decoded: WorkbenchPackage = serde_json::from_value(package.clone()).unwrap();
            assert_eq!(serde_json::to_value(decoded).unwrap(), package);
        }
        for result in [
            json!({"shapes":[{"id":"one","position":[2,3]}]}),
            json!({"kind":"future_feedback","extra":{"preserve":true}}),
            json!({"answers":[],"cancelled":false,"future_field":"preserve"}),
        ] {
            let decoded: WorkbenchResult = serde_json::from_value(result.clone()).unwrap();
            assert_eq!(serde_json::to_value(decoded).unwrap(), result);
        }
    }

    #[test]
    fn supported_questions_apply_defaults_but_future_questions_stay_opaque() {
        let wire = json!({"type":"questions","version":1,"data":{"questions":[{
            "id":"q","prompt":"Question","options":[{"value":"a","label":"A"},{"value":"b","label":"B"}]
        }]}});
        let known: WorkbenchSpec = serde_json::from_value(wire.clone()).unwrap();
        let WorkbenchData::Questions(data) = &known.data else {
            panic!("supported data should be typed")
        };
        assert!(data.questions[0].allow_other);
        let mut future = wire;
        future["version"] = json!(2);
        let unknown: WorkbenchSpec = serde_json::from_value(future.clone()).unwrap();
        assert!(matches!(unknown.data, WorkbenchData::Unknown(_)));
        assert_eq!(serde_json::to_value(unknown).unwrap(), future);
    }
}
