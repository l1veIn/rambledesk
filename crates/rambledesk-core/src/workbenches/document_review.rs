use super::*;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DocumentReviewData {
    #[schemars(length(min = 1, max = 200))]
    pub title: String,
    #[schemars(length(min = 1, max = 128))]
    pub source_version: String,
    /// Immutable source paragraphs; at most 120000 Unicode scalar values in total.
    #[schemars(length(min = 1, max = 200))]
    pub paragraphs: Vec<ReviewParagraph>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReviewParagraph {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,63}$"))]
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[schemars(length(max = 128))]
    pub label: Option<String>,
    #[schemars(length(min = 1, max = 8000))]
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict {
    Ready,
    ChangesRequested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReviewAnnotationKind {
    Comment,
    Suggestion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReviewAnnotationStatus {
    Open,
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ParagraphDecision {
    Keep,
    Revise,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReviewAnnotation {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,63}$"))]
    pub id: String,
    pub paragraph_id: String,
    /// Half-open Unicode scalar offsets (not UTF-16). All three anchor fields
    /// must be null for a whole-paragraph annotation, or present and exact.
    pub start: Option<u32>,
    pub end: Option<u32>,
    pub quote: Option<String>,
    pub kind: ReviewAnnotationKind,
    #[schemars(length(min = 1, max = 4000))]
    pub body: String,
    /// Required for suggestions, null for comments; empty means proposed deletion.
    #[schemars(length(max = 8000))]
    pub replacement: Option<String>,
    pub status: ReviewAnnotationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ParagraphMark {
    pub paragraph_id: String,
    pub decision: ParagraphDecision,
}

/// Result schema shares the exact published payload, rather than a parallel JSON shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DocumentReviewResult {
    pub source_version: String,
    pub verdict: ReviewVerdict,
    #[schemars(length(max = 500))]
    pub annotations: Vec<ReviewAnnotation>,
    #[schemars(length(max = 200))]
    pub paragraph_marks: Vec<ParagraphMark>,
}

pub(super) fn review_annotations_valid(
    data: &DocumentReviewData,
    annotations: &[ReviewAnnotation],
    marks: &[ParagraphMark],
) -> bool {
    if annotations.len() > 500 || marks.len() > data.paragraphs.len() {
        return false;
    }
    let mut ids = HashSet::new();
    for annotation in annotations {
        if !super::validation::valid_item_id(&annotation.id)
            || !ids.insert(&annotation.id)
            || annotation.body.trim().is_empty()
            || annotation.body.chars().count() > 4000
            || annotation.body.contains('\0')
        {
            return false;
        }
        let Some(paragraph) = data
            .paragraphs
            .iter()
            .find(|p| p.id == annotation.paragraph_id)
        else {
            return false;
        };
        match (&annotation.start, &annotation.end, &annotation.quote) {
            (None, None, None) => {}
            (Some(start), Some(end), Some(quote)) => {
                let chars: Vec<char> = paragraph.text.chars().collect();
                if start >= end
                    || *end as usize > chars.len()
                    || chars[*start as usize..*end as usize]
                        .iter()
                        .collect::<String>()
                        != *quote
                {
                    return false;
                }
            }
            _ => return false,
        }
        match (annotation.kind, &annotation.replacement) {
            (ReviewAnnotationKind::Comment, None) => {}
            (ReviewAnnotationKind::Suggestion, Some(value))
                if value.chars().count() <= 8000 && !value.contains('\0') => {}
            _ => return false,
        }
    }
    let mut marked = HashSet::new();
    marks.iter().all(|mark| {
        marked.insert(&mark.paragraph_id)
            && data
                .paragraphs
                .iter()
                .any(|paragraph| paragraph.id == mark.paragraph_id)
    })
}

#[cfg(test)]
mod tests {
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
                    status: ReviewAnnotationStatus::Open,
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
        let Some(WorkbenchState::DocumentReview { verdict, .. }) = &mut draft.workbench_state
        else {
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
}
