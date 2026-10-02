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
        validate_workbench(spec)
            .ok()
            .and_then(|validated| draft_result(&validated, draft))
    } else {
        None
    };
    WorkbenchPackage {
        input: spec.clone(),
        result,
    }
}

fn draft_result(
    validated: &ValidatedWorkbench<'_>,
    draft: Option<&FeedbackDraft>,
) -> Option<WorkbenchResult> {
    if draft.is_none() && !validated.kind().definition().notes_only {
        return None;
    }
    validated.result(draft.and_then(|draft| draft.workbench_state.as_ref()))
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
    let result = draft_result(&validated, draft);
    let review_input = validated.has_input(draft.and_then(|draft| draft.workbench_state.as_ref()));
    if body_markdown.trim().is_empty()
        && !validated.has_result_input(result.as_ref())
        && !review_input
    {
        return Err(WorkbenchSubmissionError::Empty);
    }
    if validated.kind().definition().require_complete && !validated.complete(result.as_ref()) {
        return Err(WorkbenchSubmissionError::Incomplete);
    }
    Ok(Some(WorkbenchPackage {
        input: spec.clone(),
        result,
    }))
}
