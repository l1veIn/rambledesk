use super::*;

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListWorkbenchesInput {
    /// Zero-based offset into the capability catalog.
    #[serde(default)]
    pub offset: usize,
    /// Defaults to 20; maximum 20. Schemas are never included in catalog pages.
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DescribeWorkbenchInput {
    #[serde(rename = "type")]
    pub kind: String,
    pub version: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkbenchSummary {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub version: u32,
    pub name: &'static str,
    pub purpose: &'static str,
    pub returns: &'static str,
    pub interaction: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkbenchListResult {
    pub workbenches: Vec<WorkbenchSummary>,
    pub next_offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkbenchDescription {
    #[serde(flatten)]
    pub summary: WorkbenchSummary,
    pub input_schema: schemars::Schema,
    pub result_schema: schemars::Schema,
    pub example: WorkbenchSpec,
    pub instructions: &'static str,
}

fn catalog() -> Vec<WorkbenchSummary> {
    WorkbenchKind::ALL
        .iter()
        .map(|kind| kind.definition())
        .filter(|definition| definition.advertised)
        .map(|definition| definition.summary)
        .collect()
}
pub fn list_workbenches(
    input: &ListWorkbenchesInput,
) -> Result<WorkbenchListResult, ApplicationError> {
    let limit = input.limit.unwrap_or(20);
    if !(1..=20).contains(&limit) {
        return Err(ApplicationError::invalid_argument("limit must be 1–20"));
    }
    let matches = catalog();
    let end = input.offset.saturating_add(limit);
    let next_offset = (end < matches.len()).then_some(end);
    Ok(WorkbenchListResult {
        workbenches: matches.into_iter().skip(input.offset).take(limit).collect(),
        next_offset,
    })
}

pub fn describe_workbench(
    input: &DescribeWorkbenchInput,
) -> Result<WorkbenchDescription, ApplicationError> {
    let kind = WorkbenchKind::resolve(&input.kind, input.version.unwrap_or(1))
        .ok_or_else(super::framework::unsupported)?;
    (kind.definition().describe)()
}
