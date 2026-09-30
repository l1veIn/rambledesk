use super::*;
use std::collections::HashSet;

/// Supported wire contracts, including compatibility-only types omitted from
/// discovery. Unknown contracts remain readable but cannot be edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkbenchKind {
    Ramble,
    Questions,
    SingleChoice,
    DocumentReview,
    WebReview,
    Terminal,
}

impl WorkbenchKind {
    pub const ALL: &[Self] = &[
        Self::Ramble,
        Self::Questions,
        Self::SingleChoice,
        Self::DocumentReview,
        Self::WebReview,
        Self::Terminal,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ramble => "ramble",
            Self::Questions => "questions",
            Self::SingleChoice => "single_choice",
            Self::DocumentReview => "document_review",
            Self::WebReview => "web_review",
            Self::Terminal => "terminal",
        }
    }

    pub fn resolve(kind: &str, version: u32) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|item| version == 1 && item.as_str() == kind)
    }

    pub fn supports_approval(self) -> bool {
        self == Self::Ramble
    }

    /// These contracts shipped with derived actions in their persisted identity.
    /// Later contracts hash their own immutable input without UI projections.
    pub fn uses_legacy_action_identity(self) -> bool {
        match self {
            Self::Ramble | Self::Questions | Self::SingleChoice => true,
            Self::DocumentReview | Self::WebReview | Self::Terminal => false,
        }
    }
}

/// Type and data are associated once, after validating immutable request input.
pub enum ValidatedWorkbench<'a> {
    Ramble(&'a RambleData),
    Questions(&'a QuestionsData),
    SingleChoice(&'a SingleChoiceData),
    DocumentReview(&'a DocumentReviewData),
    WebReview(&'a WebReviewData),
    Terminal(&'a TerminalData),
}

impl ValidatedWorkbench<'_> {
    pub fn kind(&self) -> WorkbenchKind {
        match self {
            Self::Ramble(_) => WorkbenchKind::Ramble,
            Self::Questions(_) => WorkbenchKind::Questions,
            Self::SingleChoice(_) => WorkbenchKind::SingleChoice,
            Self::DocumentReview(_) => WorkbenchKind::DocumentReview,
            Self::WebReview(_) => WorkbenchKind::WebReview,
            Self::Terminal(_) => WorkbenchKind::Terminal,
        }
    }

    /// Frozen compatibility data for existing capture targets and input hashes.
    /// New workbench types need not invent actions to pass domain validation.
    pub fn legacy_capture_actions(&self) -> Vec<ActionInput> {
        match self {
            Self::Ramble(data) => data.actions.clone(),
            Self::Questions(data) => data
                .questions
                .iter()
                .map(|question| ActionInput {
                    id: question.id.clone(),
                    instruction: question.prompt.clone(),
                })
                .collect(),
            Self::SingleChoice(data) => data
                .options
                .iter()
                .map(|option| ActionInput {
                    id: option.id.clone(),
                    instruction: option.label.clone(),
                })
                .collect(),
            Self::DocumentReview(_) | Self::WebReview(_) | Self::Terminal(_) => Vec::new(),
        }
    }
}

fn text(field: &str, value: &str, max: usize) -> Result<(), ApplicationError> {
    crate::feedback::validate_text(field, value, 1, max)?;
    if value.trim().is_empty() {
        return Err(ApplicationError::invalid_argument(format!(
            "{field} must contain visible text"
        )));
    }
    Ok(())
}

pub(super) fn valid_item_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn id<'a>(
    field: &str,
    value: &'a str,
    seen: &mut HashSet<&'a str>,
) -> Result<(), ApplicationError> {
    if !valid_item_id(value) || !seen.insert(value) {
        return Err(ApplicationError::invalid_argument(format!(
            "{field} must be unique and match ^[a-z0-9][a-z0-9_-]{{0,63}}$"
        )));
    }
    Ok(())
}

fn count(field: &str, length: usize, min: usize, max: usize) -> Result<(), ApplicationError> {
    if !(min..=max).contains(&length) {
        return Err(ApplicationError::invalid_argument(format!(
            "{field} must contain {min}–{max} items"
        )));
    }
    Ok(())
}

pub fn validate_workbench(
    spec: &WorkbenchSpec,
) -> Result<ValidatedWorkbench<'_>, ApplicationError> {
    let kind = WorkbenchKind::resolve(&spec.kind, spec.version).ok_or_else(||
        ApplicationError::invalid_argument("Unknown workbench type or unsupported version. Call list_workbenches to discover supported types."))?;
    let mut ids = HashSet::new();
    match (kind, &spec.data) {
        (WorkbenchKind::Ramble, WorkbenchData::Ramble(data)) => {
            count("ramble.actions", data.actions.len(), 1, 20)?;
            for action in &data.actions {
                id("ramble.actions.id", &action.id, &mut ids)?;
                text("ramble.actions.instruction", &action.instruction, 2000)?;
            }
            Ok(ValidatedWorkbench::Ramble(data))
        }
        (WorkbenchKind::Questions, WorkbenchData::Questions(data)) => {
            count("questions", data.questions.len(), 1, 20)?;
            for question in &data.questions {
                id("questions.id", &question.id, &mut ids)?;
                text("questions.prompt", &question.prompt, 2000)?;
                if let Some(label) = &question.label {
                    crate::feedback::validate_text("questions.label", label, 0, 40)?;
                }
                count("questions.options", question.options.len(), 2, 6)?;
                let mut values = HashSet::new();
                for option in &question.options {
                    text("questions.options.value", &option.value, 64)?;
                    if !values.insert(&option.value) {
                        return Err(ApplicationError::invalid_argument(
                            "Question option values must be unique",
                        ));
                    }
                    text("questions.options.label", &option.label, 2000)?;
                    if let Some(description) = &option.description {
                        crate::feedback::validate_text(
                            "questions.options.description",
                            description,
                            0,
                            2000,
                        )?;
                    }
                }
            }
            Ok(ValidatedWorkbench::Questions(data))
        }
        (WorkbenchKind::SingleChoice, WorkbenchData::SingleChoice(data)) => {
            text("single_choice.prompt", &data.prompt, 2000)?;
            count("single_choice.options", data.options.len(), 2, 20)?;
            for option in &data.options {
                id("single_choice.options.id", &option.id, &mut ids)?;
                text("single_choice.options.label", &option.label, 2000)?;
            }
            Ok(ValidatedWorkbench::SingleChoice(data))
        }
        (WorkbenchKind::DocumentReview, WorkbenchData::DocumentReview(data)) => {
            text("document_review.title", &data.title, 200)?;
            text("document_review.source_version", &data.source_version, 128)?;
            count("document_review.paragraphs", data.paragraphs.len(), 1, 200)?;
            let mut total = 0;
            for paragraph in &data.paragraphs {
                id("document_review.paragraphs.id", &paragraph.id, &mut ids)?;
                text("document_review.paragraphs.text", &paragraph.text, 8000)?;
                if let Some(label) = &paragraph.label {
                    crate::feedback::validate_text(
                        "document_review.paragraphs.label",
                        label,
                        0,
                        128,
                    )?;
                }
                total += paragraph.text.chars().count();
            }
            if total > 120_000 {
                return Err(ApplicationError::invalid_argument(
                    "document_review source exceeds 120000 Unicode scalar values",
                ));
            }
            Ok(ValidatedWorkbench::DocumentReview(data))
        }
        (WorkbenchKind::WebReview, WorkbenchData::WebReview(data)) => {
            text("web_review.title", &data.title, 200)?;
            text("web_review.source_version", &data.source_version, 128)?;
            if !super::web_review::web_review_url_valid(&data.url) {
                return Err(ApplicationError::invalid_argument(
                    "web_review.url must be an HTTP(S) URL without credentials, whitespace or control characters; maximum 8192 characters",
                ));
            }
            if !super::web_review::web_review_viewport_valid(&data.viewport) {
                return Err(ApplicationError::invalid_argument(
                    "web_review.viewport must be 240–7680 pixels wide and 200–4320 pixels high",
                ));
            }
            Ok(ValidatedWorkbench::WebReview(data))
        }
        (WorkbenchKind::Terminal, WorkbenchData::Terminal(data)) => {
            if !super::terminal::terminal_input_valid(data) {
                return Err(ApplicationError::invalid_argument(
                    "terminal requires cwd, optional shell and 1–20 uniquely identified, single-line suggested commands with visible title and command",
                ));
            }
            Ok(ValidatedWorkbench::Terminal(data))
        }
        _ => Err(ApplicationError::invalid_argument(
            "workbench.data does not match its type; call describe_workbench for its schema",
        )),
    }
}
