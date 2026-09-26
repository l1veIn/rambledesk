use super::*;
use schemars::json_schema;

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListWorkbenchesInput {
    /// Zero-based offset into the capability catalog.
    #[serde(default)]
    pub offset: usize,
    /// Defaults to 5; maximum 20. Schemas are never included in catalog pages.
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

impl WorkbenchKind {
    fn summary(self) -> WorkbenchSummary {
        let (name, purpose, returns, interaction) = match self {
            Self::Ramble => (
                "Ramble / 自由反馈",
                "Review or experience actions, then give free-form feedback with attachments. 自由体验、评审、语音反馈。",
                "Free-form feedback body and attachments",
                "free_feedback",
            ),
            Self::Questions => (
                "Questions / 逐项问答",
                "Ask one or more questions with options and optional custom answers. Use one question with allowOther:false for a single choice. 单选、多题问答、需求澄清。",
                "Answers associated with question id: value, label, wasCustom, index",
                "questionnaire",
            ),
            Self::SingleChoice => (
                "Single choice / 方案单选",
                "Choose one option and optionally explain the decision. 方案比较、单选、偏好选择。",
                "One selected option id with optional feedback notes",
                "single_selection",
            ),
            Self::DocumentReview => (
                "Document review / 文稿审阅",
                "Review an immutable script or speech, annotate passages and suggest revisions. 审阅脚本、发言稿或长文。",
                "Source version, explicit verdict, anchored annotations and paragraph decisions",
                "document_review",
            ),
        };
        WorkbenchSummary {
            kind: self.as_str(),
            version: 1,
            name,
            purpose,
            returns,
            interaction,
        }
    }
}

fn catalog() -> Vec<WorkbenchSummary> {
    // Compatibility contracts remain resolvable but are not offered for new work.
    [
        WorkbenchKind::Ramble,
        WorkbenchKind::Questions,
        WorkbenchKind::DocumentReview,
    ]
    .into_iter()
    .map(WorkbenchKind::summary)
    .collect()
}

pub fn list_workbenches(
    input: &ListWorkbenchesInput,
) -> Result<WorkbenchListResult, ApplicationError> {
    let limit = input.limit.unwrap_or(5);
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
        .ok_or_else(|| ApplicationError::invalid_argument("Unknown workbench type or unsupported version. Call list_workbenches to discover supported types."))?;
    let summary = kind.summary();
    let (input_schema, result_schema, data, instructions) = match kind {
        WorkbenchKind::Ramble => (
            schemars::schema_for!(RambleData),
            json_schema!({"type":"object","properties":{"kind":{"const":"free_feedback"}},"required":["kind"]}),
            WorkbenchData::Ramble(RambleData {
                actions: vec![ActionInput {
                    id: "try-it".into(),
                    instruction: "Try the new flow and describe your experience.".into(),
                }],
            }),
            "Use the shared feedback editor. Read feedback_package.markdown and attachment paths; result.kind is free_feedback.",
        ),
        WorkbenchKind::Questions => (
            schemars::schema_for!(QuestionsData),
            json_schema!({"type":"object","properties":{"answers":{"type":"array","items": schemars::schema_for!(QuestionAnswer)},"cancelled":{"const":false}},"required":["answers","cancelled"]}),
            WorkbenchData::Questions(QuestionsData {
                questions: vec![Question {
                    id: "audience".into(),
                    prompt: "Who is the main audience?".into(),
                    label: Some("Audience".into()),
                    allow_other: false,
                    options: vec![
                        QuestionOption {
                            value: "individuals".into(),
                            label: "Individuals".into(),
                            description: Some("Optimize for one person's workflow.".into()),
                        },
                        QuestionOption {
                            value: "teams".into(),
                            label: "Teams".into(),
                            description: Some("Prioritize collaboration.".into()),
                        },
                    ],
                }],
            }),
            "Use one question with allowOther:false for a single-choice decision, or multiple questions for a questionnaire. Choose one option per question; allowOther:true also permits a custom answer. Read the selected option value from result.answers for that question id. Review all answers before submitting. Every question needs an answer; cancelling produces no result. Answers are independent of optional feedback notes.",
        ),
        WorkbenchKind::SingleChoice => {
            return Err(ApplicationError::invalid_argument(
                "single_choice is a compatibility contract. For new requests, describe questions and use one question with allowOther:false. Existing single_choice requests keep their original input and result contract.",
            ));
        }
        WorkbenchKind::DocumentReview => (
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
        ),
    };
    Ok(WorkbenchDescription {
        example: WorkbenchSpec {
            kind: input.kind.clone(),
            version: 1,
            data,
        },
        summary,
        input_schema,
        result_schema,
        instructions,
    })
}
