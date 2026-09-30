use super::*;
use std::collections::HashSet;
pub(super) fn text(field: &str, value: &str, max: usize) -> Result<(), ApplicationError> {
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

pub(super) fn id<'a>(
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

pub(super) fn count(
    field: &str,
    length: usize,
    min: usize,
    max: usize,
) -> Result<(), ApplicationError> {
    if !(min..=max).contains(&length) {
        return Err(ApplicationError::invalid_argument(format!(
            "{field} must contain {min}–{max} items"
        )));
    }
    Ok(())
}
