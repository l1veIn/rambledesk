use super::*;
use std::collections::{HashMap, HashSet};
#[cfg(test)]
#[path = "table_review_tests.rs"]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TableReviewData {
    #[schemars(length(min = 1, max = 200))]
    pub title: String,
    #[schemars(length(min = 1, max = 128))]
    pub source_version: String,
    #[schemars(length(min = 1, max = 100))]
    pub columns: Vec<TableReviewColumn>,
    /// At most 20000 cells and 500000 Unicode scalars across all original values.
    #[schemars(length(min = 1, max = 1000))]
    pub rows: Vec<TableReviewRow>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TableReviewColumn {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,63}$"))]
    pub id: String,
    #[schemars(length(min = 1, max = 200))]
    pub label: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TableReviewRow {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,63}$"))]
    pub id: String,
    /// Strings in immutable column order; each may be empty, at most 4000 scalars.
    pub cells: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TableReviewChange {
    pub row_id: String,
    pub column_id: String,
    /// Suggested literal value, including an empty string to clear the cell.
    #[schemars(length(max = 4000))]
    pub value: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TableReviewComment {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,63}$"))]
    pub id: String,
    pub row_id: String,
    pub column_id: String,
    /// Markdown with shared attachment references; drafts may be empty.
    #[schemars(length(max = 4000))]
    pub body: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TableReviewState {
    #[schemars(length(max = 20000))]
    pub changes: Vec<TableReviewChange>,
    #[schemars(length(max = 500))]
    pub comments: Vec<TableReviewComment>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TableReviewResult {
    pub source_version: String,
    pub changes: Vec<TableReviewChange>,
    pub comments: Vec<TableReviewComment>,
}
pub(super) fn definition() -> WorkbenchDefinition {
    let mut definition = WorkbenchDefinition::new(
        WorkbenchSummary {
            kind: "table_review",
            version: 1,
            name: "Table review / 表格评审",
            purpose: "Review an immutable table and suggest literal cell values or leave cell comments.",
            returns: "Source version, value suggestions and comments by stable row/column IDs, with optional shared notes",
            interaction: "table_review",
        },
        describe,
    );
    definition.strict_state = true;
    definition.notes_only = true;
    definition
}
fn describe() -> Result<WorkbenchDescription, ApplicationError> {
    Ok(WorkbenchDescription {
        summary: definition().summary,
        input_schema: schemars::schema_for!(TableReviewData),
        result_schema: schemars::schema_for!(TableReviewResult),
        example: WorkbenchSpec {
            kind: "table_review".into(),
            version: 1,
            data: WorkbenchData::TableReview(TableReviewData {
                title: "Review the launch budget".into(),
                source_version: "budget-v1".into(),
                columns: vec![
                    TableReviewColumn {
                        id: "item".into(),
                        label: "Item".into(),
                    },
                    TableReviewColumn {
                        id: "amount".into(),
                        label: "Amount".into(),
                    },
                ],
                rows: vec![TableReviewRow {
                    id: "hosting".into(),
                    cells: vec!["Hosting".into(), "120".into()],
                }],
            }),
        },
        instructions: "Supply 1–100 immutable columns with unique stable IDs and visible labels, and 1–1000 rows with unique stable IDs. Each row's cells must be strings in column order with exactly one value per column. At most 20000 cells, 4000 Unicode scalars per value and 500000 scalars total. Values, including formulas, are literal text: no execution or original file changes. Suggestions reference row_id and column_id, at most one per cell and 500000 suggested scalars total; an empty value clears the cell. Published suggestions must differ from the original value. At most 500 comments with unique IDs, known cell positions and nonblank Markdown bodies up to 4000 scalars; shared attachment references are supported. Empty comment drafts may be saved but not published. Submit changed values, cell comments, shared notes or a combination. No approval verdict is supported.",
    })
}
pub(super) fn validate(data: &TableReviewData) -> Result<(), ApplicationError> {
    super::validation::text("table_review.title", &data.title, 200)?;
    super::validation::text("table_review.source_version", &data.source_version, 128)?;
    super::validation::count("table_review.columns", data.columns.len(), 1, 100)?;
    super::validation::count("table_review.rows", data.rows.len(), 1, 1000)?;
    if data.columns.len() * data.rows.len() > 20000 {
        return Err(ApplicationError::invalid_argument(
            "table_review cannot exceed 20000 cells",
        ));
    }
    let mut columns = HashSet::new();
    for column in &data.columns {
        super::validation::id("table_review.columns[].id", &column.id, &mut columns)?;
        super::validation::text("table_review.columns[].label", &column.label, 200)?;
    }
    let mut rows = HashSet::new();
    let mut total = 0usize;
    for row in &data.rows {
        super::validation::id("table_review.rows[].id", &row.id, &mut rows)?;
        if row.cells.len() != data.columns.len() {
            return Err(ApplicationError::invalid_argument(
                "table_review rows must match the immutable column count",
            ));
        }
        for value in &row.cells {
            crate::feedback::validate_text("table_review.rows[].cells[]", value, 0, 4000)?;
            total += value.chars().count();
        }
    }
    if total > 500000 {
        return Err(ApplicationError::invalid_argument(
            "table_review original values exceed 500000 characters",
        ));
    }
    Ok(())
}
fn state_valid(
    data: &TableReviewData,
    changes: &[TableReviewChange],
    comments: &[TableReviewComment],
    published: bool,
) -> bool {
    if changes.len() > 20000 || comments.len() > 500 {
        return false;
    }
    let cells: HashMap<_, _> = data
        .rows
        .iter()
        .flat_map(|row| {
            data.columns
                .iter()
                .zip(&row.cells)
                .map(move |(column, value)| ((row.id.as_str(), column.id.as_str()), value.as_str()))
        })
        .collect();
    let mut changed = HashSet::new();
    let mut total = 0usize;
    let valid_changes = changes.iter().all(|change| {
        total += change.value.chars().count();
        let position = (change.row_id.as_str(), change.column_id.as_str());
        !change.value.contains('\0')
            && change.value.chars().count() <= 4000
            && total <= 500000
            && changed.insert(position)
            && cells
                .get(&position)
                .is_some_and(|original| !published || *original != change.value.as_str())
    });
    let mut ids = HashSet::new();
    valid_changes
        && comments.iter().all(|comment| {
            super::validation::valid_item_id(&comment.id)
                && ids.insert(&comment.id)
                && cells.contains_key(&(comment.row_id.as_str(), comment.column_id.as_str()))
                && !comment.body.contains('\0')
                && comment.body.chars().count() <= 4000
                && (!published || !comment.body.trim().is_empty())
        })
}
pub(super) fn draft_valid(data: &TableReviewData, state: Option<&WorkbenchState>) -> bool {
    match state {
        None => true,
        Some(WorkbenchState::TableReview(state)) => {
            state_valid(data, &state.changes, &state.comments, false)
        }
        _ => false,
    }
}
pub(super) fn result(
    data: &TableReviewData,
    state: Option<&WorkbenchState>,
) -> Option<WorkbenchResult> {
    let (changes, comments) = match state {
        None => (vec![], vec![]),
        Some(WorkbenchState::TableReview(state))
            if state_valid(data, &state.changes, &state.comments, true) =>
        {
            (state.changes.clone(), state.comments.clone())
        }
        _ => return None,
    };
    Some(WorkbenchResult::TableReview(TableReviewResult {
        source_version: data.source_version.clone(),
        changes,
        comments,
    }))
}
pub(super) fn has_input(data: &TableReviewData, state: Option<&WorkbenchState>) -> bool {
    matches!(state, Some(WorkbenchState::TableReview(state)) if (!state.changes.is_empty() || !state.comments.is_empty()) && state_valid(data, &state.changes, &state.comments, false))
}
pub(super) fn complete(data: &TableReviewData, result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::TableReview(result)) if result.source_version == data.source_version && state_valid(data, &result.changes, &result.comments, true))
}
pub(super) fn result_has_input(result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::TableReview(result)) if !result.changes.is_empty() || !result.comments.is_empty())
}
pub(super) fn legacy_actions(_: &TableReviewData) -> Vec<ActionInput> {
    vec![]
}
