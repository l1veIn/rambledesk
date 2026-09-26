use super::*;

/// Parsed interaction content from a supported draft format. Storage owns the
/// serialized envelope codec; core makes decisions only over domain values.
pub struct FeedbackDraft {
    pub workbench_state: Option<WorkbenchState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkbenchSubmissionError {
    Empty,
    Incomplete,
    Unsupported,
}

/// Build a package without publishing any draft answers for cancellation.
pub fn workbench_package(
    spec: &WorkbenchSpec,
    draft: Option<&FeedbackDraft>,
    submitted: bool,
) -> WorkbenchPackage {
    let result = if submitted {
        draft.and_then(|draft| workbench_result(spec, draft.workbench_state.as_ref()))
    } else {
        None
    };
    WorkbenchPackage {
        input: spec.clone(),
        result,
    }
}

/// Pure submission policy, invoked by the repository on its transaction's
/// immutable request and draft snapshot before freezing publication.
pub fn prepare_feedback_submission(
    spec: Option<&WorkbenchSpec>,
    draft: Option<&FeedbackDraft>,
    body_markdown: &str,
) -> Result<Option<WorkbenchPackage>, WorkbenchSubmissionError> {
    let Some(spec) = spec else {
        return if body_markdown.trim().is_empty() {
            Err(WorkbenchSubmissionError::Empty)
        } else {
            Ok(None)
        };
    };
    let validated = validate_workbench(spec).map_err(|_| WorkbenchSubmissionError::Unsupported)?;
    let package = workbench_package(spec, draft, true);
    let review_input = match draft.and_then(|item| item.workbench_state.as_ref()) {
        Some(WorkbenchState::DocumentReview {
            verdict,
            annotations,
            paragraph_marks,
        }) if validated.kind() == WorkbenchKind::DocumentReview => {
            verdict.is_some()
                || annotations
                    .iter()
                    .any(|annotation| !annotation.body.trim().is_empty())
                || !paragraph_marks.is_empty()
        }
        _ => false,
    };
    if body_markdown.trim().is_empty() && !workbench_result_has_input(&package) && !review_input {
        return Err(WorkbenchSubmissionError::Empty);
    }
    if validated.kind() != WorkbenchKind::Ramble && !workbench_result_complete(&package) {
        return Err(WorkbenchSubmissionError::Incomplete);
    }
    Ok(Some(package))
}
