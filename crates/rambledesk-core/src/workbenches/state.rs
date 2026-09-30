use super::*;

/// Pi questionnaire answer: stable option value, display label and explicit custom input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct QuestionAnswer {
    pub id: String,
    pub value: String,
    pub label: String,
    #[serde(rename = "wasCustom")]
    pub was_custom: bool,
    /// One-based option index. Absent for custom input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub index: Option<u32>,
}

/// Canonical interaction state, separate from the rich-text document.
/// Both fields share one draft revision and are saved atomically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkbenchState {
    Questions {
        answers: Vec<QuestionAnswer>,
    },
    SingleChoice {
        selected_option_id: Option<String>,
    },
    DocumentReview {
        verdict: Option<ReviewVerdict>,
        annotations: Vec<ReviewAnnotation>,
        paragraph_marks: Vec<ParagraphMark>,
    },
    WebReview {
        annotations: Vec<WebReviewAnnotation>,
    },
    Terminal {
        sessions: Vec<TerminalTrialSession>,
    },
}

/// Resolve values against immutable request data; never trust client display labels.
pub fn workbench_result(
    spec: &WorkbenchSpec,
    state: Option<&WorkbenchState>,
) -> Option<WorkbenchResult> {
    match (validate_workbench(spec).ok()?, state) {
        (ValidatedWorkbench::Ramble(_), _) => Some(WorkbenchResult::Ramble {
            kind: "free_feedback".into(),
        }),
        (ValidatedWorkbench::Questions(data), Some(WorkbenchState::Questions { answers })) => {
            if answers.len() > data.questions.len() {
                return None;
            }
            let mut seen = std::collections::HashSet::new();
            let mut resolved = Vec::new();
            for answer in answers {
                if !seen.insert(&answer.id) {
                    return None;
                }
                let question = data
                    .questions
                    .iter()
                    .find(|question| question.id == answer.id)?;
                if answer.was_custom {
                    if !question.allow_other
                        || answer.value.chars().count() > 4000
                        || answer.value.contains('\0')
                    {
                        return None;
                    }
                    if !answer.value.trim().is_empty() {
                        let value = answer.value.trim().to_owned();
                        resolved.push(QuestionAnswer {
                            id: answer.id.clone(),
                            label: value.clone(),
                            value,
                            was_custom: true,
                            index: None,
                        });
                    }
                } else {
                    let (index, option) = question
                        .options
                        .iter()
                        .enumerate()
                        .find(|(_, option)| option.value == answer.value)?;
                    resolved.push(QuestionAnswer {
                        id: answer.id.clone(),
                        value: option.value.clone(),
                        label: option.label.clone(),
                        was_custom: false,
                        index: Some(index as u32 + 1),
                    });
                }
            }
            resolved.sort_by_key(|answer| {
                data.questions
                    .iter()
                    .position(|question| question.id == answer.id)
            });
            Some(WorkbenchResult::Questions {
                answers: resolved,
                cancelled: false,
            })
        }
        (
            ValidatedWorkbench::SingleChoice(data),
            Some(WorkbenchState::SingleChoice { selected_option_id }),
        ) => {
            if selected_option_id
                .as_ref()
                .is_some_and(|id| !data.options.iter().any(|option| option.id == *id))
            {
                return None;
            }
            Some(WorkbenchResult::SingleChoice {
                status: if selected_option_id.is_some() {
                    AnswerStatus::Answered
                } else {
                    AnswerStatus::Unanswered
                },
                selected_option_id: selected_option_id.clone(),
            })
        }
        (
            ValidatedWorkbench::DocumentReview(data),
            Some(WorkbenchState::DocumentReview {
                verdict: Some(verdict),
                annotations,
                paragraph_marks,
            }),
        ) if super::document_review::review_annotations_valid(
            data,
            annotations,
            paragraph_marks,
        ) =>
        {
            Some(WorkbenchResult::DocumentReview(DocumentReviewResult {
                source_version: data.source_version.clone(),
                verdict: *verdict,
                annotations: annotations.clone(),
                paragraph_marks: paragraph_marks.clone(),
            }))
        }
        (ValidatedWorkbench::WebReview(data), Some(WorkbenchState::WebReview { annotations }))
            if super::web_review::web_review_annotations_valid(annotations) =>
        {
            Some(WorkbenchResult::WebReview(WebReviewResult {
                source_version: data.source_version.clone(),
                annotations: annotations.clone(),
            }))
        }
        // General feedback can be submitted without selecting any elements.
        (ValidatedWorkbench::WebReview(data), None) => {
            Some(WorkbenchResult::WebReview(WebReviewResult {
                source_version: data.source_version.clone(),
                annotations: Vec::new(),
            }))
        }
        (ValidatedWorkbench::Terminal(_), Some(WorkbenchState::Terminal { sessions }))
            if super::terminal::terminal_sessions_valid(sessions) =>
        {
            Some(WorkbenchResult::Terminal(TerminalResult {
                sessions: sessions.clone(),
            }))
        }
        (ValidatedWorkbench::Terminal(_), None) => {
            Some(WorkbenchResult::Terminal(TerminalResult {
                sessions: Vec::new(),
            }))
        }
        _ => None,
    }
}

/// Whether a validated result contains human input, independently of completeness.
/// A Ramble marker describes the renderer; it is not itself human input.
pub fn workbench_result_has_input(package: &WorkbenchPackage) -> bool {
    match &package.result {
        Some(WorkbenchResult::Questions { answers, .. }) => !answers.is_empty(),
        Some(WorkbenchResult::SingleChoice {
            selected_option_id, ..
        }) => selected_option_id.is_some(),
        Some(WorkbenchResult::DocumentReview(_)) => true,
        Some(WorkbenchResult::WebReview(result)) => !result.annotations.is_empty(),
        // Startup output is context, not a reviewer's opinion. Terminal trials
        // require feedback in the shared body, independently of captured logs.
        Some(WorkbenchResult::Terminal(_)) => false,
        _ => false,
    }
}

pub fn workbench_result_complete(package: &WorkbenchPackage) -> bool {
    match (&package.input.data, &package.result) {
        (WorkbenchData::Questions(data), Some(WorkbenchResult::Questions { answers, .. })) => {
            answers.len() == data.questions.len()
        }
        (
            WorkbenchData::SingleChoice(_),
            Some(WorkbenchResult::SingleChoice {
                status: AnswerStatus::Answered,
                ..
            }),
        ) => true,
        (WorkbenchData::DocumentReview(_), Some(WorkbenchResult::DocumentReview(_))) => true,
        (WorkbenchData::WebReview(_), Some(WorkbenchResult::WebReview(_))) => true,
        (WorkbenchData::Terminal(_), Some(WorkbenchResult::Terminal(_))) => true,
        _ => false,
    }
}
