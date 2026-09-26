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
