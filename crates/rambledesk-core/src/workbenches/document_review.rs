use super::*;
pub(super) fn draft_valid(data: &DocumentReviewData, state: Option<&WorkbenchState>) -> bool {
    match state {
        None => true,
        Some(WorkbenchState::DocumentReview {
            annotations,
            paragraph_marks,
            ..
        }) => review_annotations_valid(data, annotations, paragraph_marks),
        _ => false,
    }
}
pub(super) fn result_has_input(result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::DocumentReview(_)))
}
use super::validation::{count, id, text};
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
pub enum ParagraphDecision {
    Keep,
    Revise,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema, TS)]
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
}

impl<'de> Deserialize<'de> for ReviewAnnotation {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Accept the retired field only while decoding older data. The live
        // model and new results have no status; all other unknown fields fail.
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireAnnotation {
            id: String,
            paragraph_id: String,
            start: Option<u32>,
            end: Option<u32>,
            quote: Option<String>,
            kind: ReviewAnnotationKind,
            body: String,
            replacement: Option<String>,
            #[serde(default, rename = "status")]
            _legacy_status: Option<serde::de::IgnoredAny>,
        }
        let wire = WireAnnotation::deserialize(deserializer)?;
        Ok(Self {
            id: wire.id,
            paragraph_id: wire.paragraph_id,
            start: wire.start,
            end: wire.end,
            quote: wire.quote,
            kind: wire.kind,
            body: wire.body,
            replacement: wire.replacement,
        })
    }
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

pub(super) fn definition() -> WorkbenchDefinition {
    let (name, purpose, returns, interaction) = (
        "Document review / 文稿审阅",
        "Review an immutable script or speech, annotate passages and suggest revisions. 审阅脚本、发言稿或长文。",
        "Source version, explicit verdict, anchored annotations and paragraph decisions",
        "document_review",
    );
    let mut definition = WorkbenchDefinition::new(
        WorkbenchSummary {
            kind: "document_review",
            version: 1,
            name,
            purpose,
            returns,
            interaction,
        },
        describe,
    );

    definition.validate_saved_draft = false;
    definition
}
fn describe() -> Result<WorkbenchDescription, ApplicationError> {
    let (input_schema, result_schema, data, instructions) = (
        schemars::schema_for!(DocumentReviewData),
        schemars::schema_for!(DocumentReviewResult),
        WorkbenchData::DocumentReview(DocumentReviewData {
            title: "Opening speech".into(),
            source_version: "draft-1".into(),
            paragraphs: vec![
                ReviewParagraph {
                    id: "opening".into(),
                    label: Some("Opening".into()),
                    text: "Thank you for joining us today.".into(),
                },
                ReviewParagraph {
                    id: "purpose".into(),
                    label: Some("Purpose".into()),
                    text: "We will decide what to build next.".into(),
                },
            ],
        }),
        "The source is immutable. Annotate whole paragraphs or exact text ranges; start/end are half-open Unicode scalar offsets, not UTF-16, and quote must exactly match the source slice. Suggestion replacement may be empty to propose deletion; comments have null replacement. All annotations need nonblank body text. Select an explicit ready or changes_requested verdict before submitting; paragraph marks are optional. The verdict is review feedback, never authorization to execute changes. At most 200 paragraphs and 120000 source characters; at most 500 annotations.",
    );
    Ok(WorkbenchDescription {
        summary: definition().summary,
        example: WorkbenchSpec {
            kind: "document_review".into(),
            version: 1,
            data,
        },
        input_schema,
        result_schema,
        instructions,
    })
}
pub(super) fn validate(data: &DocumentReviewData) -> Result<(), ApplicationError> {
    let mut ids = std::collections::HashSet::new();

    text("document_review.title", &data.title, 200)?;
    text("document_review.source_version", &data.source_version, 128)?;
    count("document_review.paragraphs", data.paragraphs.len(), 1, 200)?;
    let mut total = 0;
    for paragraph in &data.paragraphs {
        id("document_review.paragraphs.id", &paragraph.id, &mut ids)?;
        text("document_review.paragraphs.text", &paragraph.text, 8000)?;
        if let Some(label) = &paragraph.label {
            crate::feedback::validate_text("document_review.paragraphs.label", label, 0, 128)?;
        }
        total += paragraph.text.chars().count();
    }
    if total > 120_000 {
        return Err(ApplicationError::invalid_argument(
            "document_review source exceeds 120000 Unicode scalar values",
        ));
    }
    Ok(())
}

pub(super) fn result(
    data: &DocumentReviewData,
    state: Option<&WorkbenchState>,
) -> Option<WorkbenchResult> {
    let Some(WorkbenchState::DocumentReview {
        verdict: Some(verdict),
        annotations,
        paragraph_marks,
    }) = state
    else {
        return None;
    };
    if !review_annotations_valid(data, annotations, paragraph_marks) {
        return None;
    }
    Some(WorkbenchResult::DocumentReview(DocumentReviewResult {
        source_version: data.source_version.clone(),
        verdict: *verdict,
        annotations: annotations.clone(),
        paragraph_marks: paragraph_marks.clone(),
    }))
}
pub(super) fn has_input(_: &DocumentReviewData, state: Option<&WorkbenchState>) -> bool {
    matches!(state,Some(WorkbenchState::DocumentReview{verdict,annotations,paragraph_marks}) if verdict.is_some() || annotations.iter().any(|a|!a.body.trim().is_empty()) || !paragraph_marks.is_empty())
}
pub(super) fn complete(_: &DocumentReviewData, result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::DocumentReview(_)))
}
pub(super) fn legacy_actions(_: &DocumentReviewData) -> Vec<ActionInput> {
    Vec::new()
}
