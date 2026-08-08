use std::collections::HashSet;

use uuid::Uuid;

use super::{ApplicationError, RequestFeedbackInput};

pub(super) fn validate_request_input(input: &RequestFeedbackInput) -> Result<(), ApplicationError> {
    let host_id = input.host_id.as_deref().unwrap_or("generic");
    validate_text("host_id", host_id, 1, 64)?;
    validate_text("host_session_id", &input.host_session_id, 1, 256)?;
    if let Some(title) = input.title.as_deref() {
        validate_text("title", title, 1, 160)?;
        if title.trim().is_empty() {
            return Err(ApplicationError::invalid_argument(
                "title must contain visible characters",
            ));
        }
    }
    validate_text("what_happened", &input.what_happened, 1, 12_000)?;
    match (input.allow_finish, input.final_summary.as_deref()) {
        (true, Some(summary)) => validate_text("final_summary", summary, 1, 12_000)?,
        (true, None) => {
            return Err(ApplicationError::invalid_argument(
                "final_summary is required when allow_finish is true",
            ));
        }
        (false, Some(_)) => {
            return Err(ApplicationError::invalid_argument(
                "final_summary requires allow_finish to be true",
            ));
        }
        (false, None) => {}
    }

    if let Some(request_id) = input.request_id.as_deref() {
        canonical_uuid(request_id, "request_id")?;
    }
    if let Some(source_hint) = input.source_hint.as_deref() {
        validate_text("source_hint", source_hint, 1, 4_096)?;
    }

    if !(1..=20).contains(&input.actions.len()) {
        return Err(ApplicationError::invalid_argument(
            "actions must contain between 1 and 20 items",
        ));
    }
    let mut action_ids = HashSet::with_capacity(input.actions.len());
    for action in &input.actions {
        if !valid_action_id(&action.id) {
            return Err(ApplicationError::invalid_argument(
                "action id must match ^[a-z0-9][a-z0-9_-]{0,63}$",
            ));
        }
        if !action_ids.insert(action.id.as_str()) {
            return Err(ApplicationError::invalid_argument(
                "action ids must be unique within a request",
            ));
        }
        validate_text("action.instruction", &action.instruction, 1, 2_000)?;
    }

    if input.context_refs.len() > 20 {
        return Err(ApplicationError::invalid_argument(
            "context_refs cannot contain more than 20 items",
        ));
    }
    for context_ref in &input.context_refs {
        validate_text("context_ref.label", &context_ref.label, 1, 256)?;
        validate_text("context_ref.uri", &context_ref.uri, 1, 4_096)?;
    }

    if input.attachments.len() > crate::MAX_ATTACHMENT_COUNT {
        return Err(ApplicationError::invalid_argument(format!(
            "attachments cannot contain more than {} items",
            crate::MAX_ATTACHMENT_COUNT
        )));
    }
    for attachment in &input.attachments {
        match (&attachment.markdown, &attachment.contents_base64) {
            (Some(_), None) | (None, Some(_)) => {}
            _ => {
                return Err(ApplicationError::invalid_argument(
                    "each attachment must provide exactly one of markdown or contents_base64",
                ));
            }
        }
    }

    Ok(())
}

pub(crate) fn canonical_uuid(value: &str, field: &str) -> Result<String, ApplicationError> {
    Uuid::parse_str(value)
        .map(|value| value.hyphenated().to_string())
        .map_err(|_| ApplicationError::invalid_argument(format!("{field} must be a UUID")))
}

pub(crate) fn validate_text(
    field: &str,
    value: &str,
    min: usize,
    max: usize,
) -> Result<(), ApplicationError> {
    let length = value.chars().count();
    if !(min..=max).contains(&length) {
        return Err(ApplicationError::invalid_argument(format!(
            "{field} must contain between {min} and {max} characters"
        )));
    }
    if value.contains('\0') {
        return Err(ApplicationError::invalid_argument(format!(
            "{field} cannot contain NUL"
        )));
    }
    Ok(())
}

pub(super) fn valid_action_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0].is_ascii_lowercase_or_digit()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase_or_digit() || matches!(byte, b'_' | b'-'))
}

trait AsciiLowercaseOrDigit {
    fn is_ascii_lowercase_or_digit(&self) -> bool;
}

impl AsciiLowercaseOrDigit for u8 {
    fn is_ascii_lowercase_or_digit(&self) -> bool {
        self.is_ascii_lowercase() || self.is_ascii_digit()
    }
}
