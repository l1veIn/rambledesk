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

fn catalog() -> Vec<WorkbenchSummary> {
    vec![
        WorkbenchSummary {
            kind: "ramble",
            version: 1,
            name: "Ramble / 自由反馈",
            purpose: "Review or experience actions, then give free-form feedback with attachments. 自由体验、评审、语音反馈。",
            returns: "Free-form feedback body and attachments",
            interaction: "free_feedback",
        },
        WorkbenchSummary {
            kind: "questions",
            version: 1,
            name: "Questions / 逐项问答",
            purpose: "Ask one or more questions with options and optional custom answers, one at a time. 选项问答、需求澄清。",
            returns: "Answers associated with question id: value, label, wasCustom, index",
            interaction: "questionnaire",
        },
        WorkbenchSummary {
            kind: "single_choice",
            version: 1,
            name: "Single choice / 方案单选",
            purpose: "Choose one option and optionally explain the decision. 方案比较、单选、偏好选择。",
            returns: "One selected option id with optional feedback notes",
            interaction: "single_selection",
        },
    ]
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
    let summary = catalog().into_iter().find(|item| item.kind == input.kind && input.version.unwrap_or(1) == item.version)
        .ok_or_else(|| ApplicationError::invalid_argument("Unknown workbench type or unsupported version. Call list_workbenches to discover supported types."))?;
    let (input_schema, result_schema, data, instructions) = match input.kind.as_str() {
        "ramble" => (
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
        "questions" => (
            schemars::schema_for!(QuestionsData),
            json_schema!({"type":"object","properties":{"answers":{"type":"array","items": schemars::schema_for!(QuestionAnswer)},"cancelled":{"const":false}},"required":["answers","cancelled"]}),
            WorkbenchData::Questions(QuestionsData {
                questions: vec![
                    Question {
                        id: "audience".into(),
                        prompt: "Who is the main audience?".into(),
                        label: Some("Audience".into()),
                        allow_other: true,
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
                    },
                    Question {
                        id: "scope".into(),
                        prompt: "What should we build first?".into(),
                        label: Some("Scope".into()),
                        allow_other: true,
                        options: vec![
                            QuestionOption {
                                value: "feedback".into(),
                                label: "Feedback".into(),
                                description: None,
                            },
                            QuestionOption {
                                value: "review".into(),
                                label: "Review".into(),
                                description: None,
                            },
                        ],
                    },
                ],
            }),
            "Pi-style questionnaire: choose one option per question or write a custom answer when allowOther is true. Review all answers before submitting. Every question needs an answer; cancelling produces no result. Answers are independent of optional feedback notes.",
        ),
        "single_choice" => (
            schemars::schema_for!(SingleChoiceData),
            json_schema!({"type":"object","properties":{"status":{"enum":["answered","unanswered"]},"selected_option_id":{"type":["string","null"]}},"required":["status","selected_option_id"]}),
            WorkbenchData::SingleChoice(SingleChoiceData {
                prompt: "Which layout should we use?".into(),
                options: vec![
                    ChoiceOption {
                        id: "compact".into(),
                        label: "Compact layout".into(),
                    },
                    ChoiceOption {
                        id: "spacious".into(),
                        label: "Spacious layout".into(),
                    },
                ],
            }),
            "Choose one option. Selection is saved independently from optional feedback notes. A selection is required to submit; free text never implies a selection.",
        ),
        _ => unreachable!(),
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
