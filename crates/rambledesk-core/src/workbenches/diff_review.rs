use super::*;
use std::collections::{HashMap, HashSet};
#[path = "diff_review_parser.rs"]
mod parser;
#[cfg(test)]
#[path = "diff_review_tests.rs"]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DiffReviewData {
    #[schemars(length(min = 1, max = 200))]
    pub title: String,
    #[schemars(length(min = 1, max = 128))]
    pub source_version: String,
    #[schemars(length(min = 1, max = 100))]
    pub files: Vec<DiffReviewFile>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DiffReviewFile {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,63}$"))]
    pub id: String,
    #[schemars(length(min = 1, max = 2000))]
    pub old_path: String,
    #[schemars(length(min = 1, max = 2000))]
    pub new_path: String,
    /// Immutable, single-file unified diff. At most 120000 Unicode scalars per file, 500000 total.
    #[schemars(length(min = 1, max = 120000))]
    pub diff: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum DiffReviewSide {
    Old,
    New,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DiffReviewAnchor {
    pub file_id: String,
    /// Zero-based index in the immutable file's hunk sequence.
    pub hunk_index: u32,
    pub side: DiffReviewSide,
    /// Both null for a whole hunk, otherwise a one-based inclusive line range on this side.
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DiffReviewComment {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,63}$"))]
    pub id: String,
    pub anchor: DiffReviewAnchor,
    /// Drafts may be empty while editing; published comments must contain visible text.
    #[schemars(length(max = 4000))]
    pub body: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DiffReviewState {
    #[schemars(length(max = 500))]
    pub comments: Vec<DiffReviewComment>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DiffReviewResult {
    pub source_version: String,
    #[schemars(length(max = 500))]
    pub comments: Vec<DiffReviewComment>,
}
pub(super) fn definition() -> WorkbenchDefinition {
    let mut definition = WorkbenchDefinition::new(
        WorkbenchSummary {
            kind: "diff_review",
            version: 1,
            name: "Diff review / 差异评审",
            purpose: "Review agent-supplied immutable unified diffs by file and comment on lines, ranges or hunks.",
            returns: "Source version and comments anchored to exact old/new sides and hunks, with optional shared notes",
            interaction: "diff_review",
        },
        describe,
    );
    definition.strict_state = true;
    definition.notes_only = true;
    definition
}
fn describe() -> Result<WorkbenchDescription, ApplicationError> {
    Ok(WorkbenchDescription {
        summary: definition().summary, input_schema: schemars::schema_for!(DiffReviewData), result_schema: schemars::schema_for!(DiffReviewResult),
        example: WorkbenchSpec { kind: "diff_review".into(), version: 1, data: WorkbenchData::DiffReview(DiffReviewData {
            title: "Review the greeting change".into(), source_version: "commit-abc123".into(), files: vec![DiffReviewFile { id: "greeting".into(), old_path: "src/greet.ts".into(), new_path: "src/greet.ts".into(), diff: "--- a/src/greet.ts\n+++ b/src/greet.ts\n@@ -1 +1 @@\n-export const greeting = 'Hi'\n+export const greeting = 'Hello'\n".into() }],
        }) },
        instructions: "Supply 1–100 files with unique stable IDs, old/new path labels (/dev/null for added/deleted files), and immutable single-file unified diffs. Each file needs at least one valid @@ -oldStart[,count] +newStart[,count] @@ hunk. Omitted counts mean one; a start of zero is permitted only with count zero. Context/deletion/addition counts must exactly match each hunk header; no-newline markers do not advance line numbers. Binary or combined diffs are unsupported. At most 120000 Unicode scalars per file and 500000 total. Comments reference file_id, zero-based hunk_index and old/new side; start_line/end_line are both null for a hunk or an inclusive one-based range entirely within that side of the hunk. At most 500 comments with unique IDs and nonblank bodies up to 4000 Unicode scalars. Submit comments, shared overall feedback notes, or both. No approval verdict or shortcut finish is supported.",
    })
}
pub(super) fn validate(data: &DiffReviewData) -> Result<(), ApplicationError> {
    super::validation::text("diff_review.title", &data.title, 200)?;
    super::validation::text("diff_review.source_version", &data.source_version, 128)?;
    super::validation::count("diff_review.files", data.files.len(), 1, 100)?;
    let mut ids = HashSet::new();
    let mut total = 0usize;
    for file in &data.files {
        super::validation::id("diff_review.files[].id", &file.id, &mut ids)?;
        super::validation::text("diff_review.files[].old_path", &file.old_path, 2000)?;
        super::validation::text("diff_review.files[].new_path", &file.new_path, 2000)?;
        super::validation::text("diff_review.files[].diff", &file.diff, 120000)?;
        total += file.diff.chars().count();
        if total > 500000 || parser::parse(&file.diff).is_none() {
            return Err(ApplicationError::invalid_argument(
                "diff_review requires valid single-file unified diffs with exact hunk counts, at most 500000 characters total",
            ));
        }
    }
    Ok(())
}
fn comments_valid(data: &DiffReviewData, comments: &[DiffReviewComment], published: bool) -> bool {
    if comments.len() > 500 {
        return false;
    }
    let hunks: HashMap<_, _> = data
        .files
        .iter()
        .filter_map(|file| Some((file.id.as_str(), parser::parse(&file.diff)?)))
        .collect();
    let mut ids = HashSet::new();
    comments.iter().all(|comment| {
        let anchor = &comment.anchor;
        super::validation::valid_item_id(&comment.id)
            && ids.insert(&comment.id)
            && !comment.body.contains('\0')
            && comment.body.chars().count() <= 4000
            && (!published || !comment.body.trim().is_empty())
            && hunks
                .get(anchor.file_id.as_str())
                .and_then(|file| file.get(anchor.hunk_index as usize))
                .is_some_and(|hunk| {
                    let (start, count) = match anchor.side {
                        DiffReviewSide::Old => hunk.old,
                        DiffReviewSide::New => hunk.new,
                    };
                    match (anchor.start_line, anchor.end_line) {
                        (None, None) => true,
                        (Some(first), Some(last)) => {
                            count > 0
                                && first >= start
                                && first > 0
                                && last >= first
                                && last - start < count
                        }
                        _ => false,
                    }
                })
    })
}
pub(super) fn draft_valid(data: &DiffReviewData, state: Option<&WorkbenchState>) -> bool {
    match state {
        None => true,
        Some(WorkbenchState::DiffReview(state)) => comments_valid(data, &state.comments, false),
        _ => false,
    }
}
pub(super) fn result(
    data: &DiffReviewData,
    state: Option<&WorkbenchState>,
) -> Option<WorkbenchResult> {
    let comments = match state {
        None => vec![],
        Some(WorkbenchState::DiffReview(state)) if comments_valid(data, &state.comments, true) => {
            state.comments.clone()
        }
        _ => return None,
    };
    Some(WorkbenchResult::DiffReview(DiffReviewResult {
        source_version: data.source_version.clone(),
        comments,
    }))
}
pub(super) fn has_input(data: &DiffReviewData, state: Option<&WorkbenchState>) -> bool {
    matches!(state, Some(WorkbenchState::DiffReview(state)) if !state.comments.is_empty() && comments_valid(data, &state.comments, false))
}
pub(super) fn complete(data: &DiffReviewData, result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::DiffReview(result)) if result.source_version == data.source_version && comments_valid(data, &result.comments, true))
}
pub(super) fn result_has_input(result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::DiffReview(result)) if !result.comments.is_empty())
}
pub(super) fn legacy_actions(_: &DiffReviewData) -> Vec<ActionInput> {
    vec![]
}
