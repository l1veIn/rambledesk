use super::*;
pub(super) fn draft_valid(data: &DocumentReviewData, state: Option<&WorkbenchState>) -> bool {
    match state {
        None => true,
        Some(WorkbenchState::DocumentReview { annotations, .. }) => {
            review_annotations_valid(data, annotations)
        }
        _ => false,
    }
}
pub(super) fn result_has_input(result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::DocumentReview(_)))
}
use super::validation::text;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DocumentReviewData {
    #[schemars(length(min = 1, max = 200))]
    pub title: String,
    #[schemars(length(min = 1, max = 128))]
    pub source_version: String,
    /// Immutable raw Markdown; at most 120000 Unicode scalar values. Whitespace,
    /// line endings and Markdown syntax are preserved and count toward offsets.
    #[schemars(length(min = 1, max = 120000))]
    pub markdown: String,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReviewAnnotation {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,63}$"))]
    pub id: String,
    /// Half-open Unicode scalar offsets into the raw Markdown, not UTF-16 or
    /// rendered text. All three anchor fields must be null for a whole-document
    /// annotation, or present with quote exactly matching the source slice.
    pub start: Option<u32>,
    pub end: Option<u32>,
    pub quote: Option<String>,
    pub kind: ReviewAnnotationKind,
    /// Nonblank for comments; optional explanation for suggestions (may be empty).
    #[schemars(length(max = 4000))]
    pub body: String,
    /// Required for suggestions, null for comments; empty means proposed deletion.
    #[schemars(length(max = 8000))]
    pub replacement: Option<String>,
}

/// Result schema shares the exact published payload, rather than a parallel JSON shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DocumentReviewResult {
    pub source_version: String,
    pub verdict: ReviewVerdict,
    #[schemars(length(max = 500))]
    pub annotations: Vec<ReviewAnnotation>,
}

pub(super) fn review_annotations_valid(
    data: &DocumentReviewData,
    annotations: &[ReviewAnnotation],
) -> bool {
    if annotations.len() > 500 {
        return false;
    }
    let source: Vec<char> = data.markdown.chars().collect();
    let mut ids = HashSet::new();
    for annotation in annotations {
        if !super::validation::valid_item_id(&annotation.id)
            || !ids.insert(&annotation.id)
            || annotation.body.chars().count() > 4000
            || annotation.body.contains('\0')
        {
            return false;
        }
        match (&annotation.start, &annotation.end, &annotation.quote) {
            (None, None, None) => {}
            (Some(start), Some(end), Some(quote)) => {
                if start >= end
                    || *end as usize > source.len()
                    || !source[*start as usize..*end as usize]
                        .iter()
                        .copied()
                        .eq(quote.chars())
                {
                    return false;
                }
            }
            _ => return false,
        }
        match (annotation.kind, &annotation.replacement) {
            (ReviewAnnotationKind::Comment, None) if !annotation.body.trim().is_empty() => {}
            (ReviewAnnotationKind::Suggestion, Some(value))
                if value.chars().count() <= 8000 && !value.contains('\0') => {}
            _ => return false,
        }
    }
    true
}

pub(super) fn definition() -> WorkbenchDefinition {
    let (name, purpose, returns, interaction) = (
        "Markdown document review / Markdown 文档审阅",
        "Review an immutable Markdown document, annotate source ranges and suggest revisions. 审阅 Markdown 文档，批注原文并建议修改。",
        "Source version, explicit verdict and annotations anchored to raw Markdown",
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
            title: "First-run guide proposal".into(),
            source_version: "draft-1".into(),
            markdown: "# First-run guide\n\nComplete one feedback task before exploring settings.\n\n## Next step\n\nTry the guide and note any confusing instructions.".into(),
        }),
        "The raw Markdown source is immutable and is not normalized. Annotate the whole document with null start/end/quote, or an exact raw Markdown range with half-open Unicode scalar offsets (not UTF-16 or rendered-text offsets); quote must exactly match that source slice, including Markdown syntax and line endings. Suggestions require a replacement string, which may be empty to propose deletion; their body is an optional explanation and may be empty or whitespace. Comments have null replacement. Suggestions are review advice, not mechanical patches. Partial raw Markdown quotes may omit surrounding formatting; preserve document structure when revising. Before submitting, comments need nonblank body text and an explicit ready or changes_requested verdict is required. The verdict is review feedback, never authorization to execute changes. At most 120000 source Unicode scalar values and 500 annotations; body at most 4000 and replacement at most 8000 scalar values.",
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
    text("document_review.title", &data.title, 200)?;
    text("document_review.source_version", &data.source_version, 128)?;
    text("document_review.markdown", &data.markdown, 120_000)?;
    Ok(())
}

pub(super) fn result(
    data: &DocumentReviewData,
    state: Option<&WorkbenchState>,
) -> Option<WorkbenchResult> {
    let Some(WorkbenchState::DocumentReview {
        verdict: Some(verdict),
        annotations,
    }) = state
    else {
        return None;
    };
    if !review_annotations_valid(data, annotations) {
        return None;
    }
    Some(WorkbenchResult::DocumentReview(DocumentReviewResult {
        source_version: data.source_version.clone(),
        verdict: *verdict,
        annotations: annotations.clone(),
    }))
}
pub(super) fn has_input(_: &DocumentReviewData, state: Option<&WorkbenchState>) -> bool {
    matches!(state,Some(WorkbenchState::DocumentReview{verdict,annotations}) if verdict.is_some() || annotations.iter().any(|a|!a.body.trim().is_empty() || (a.kind == ReviewAnnotationKind::Suggestion && a.replacement.is_some())))
}
pub(super) fn complete(_: &DocumentReviewData, result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::DocumentReview(_)))
}
pub(super) fn legacy_actions(_: &DocumentReviewData) -> Vec<ActionInput> {
    Vec::new()
}
