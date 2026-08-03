use super::{ApplicationError, FeedbackRequestSummary};

#[derive(Debug)]
pub(super) struct ListCursor {
    pub(super) updated_at: String,
    pub(super) request_id: String,
}

pub(super) fn encode_list_cursor(
    summary: &FeedbackRequestSummary,
) -> Result<String, ApplicationError> {
    Ok(hex::encode(format!(
        "{}\0{}",
        summary.updated_at, summary.request_id
    )))
}

pub(super) fn decode_list_cursor(value: &str) -> Result<ListCursor, ApplicationError> {
    let bytes =
        hex::decode(value).map_err(|_| ApplicationError::invalid_argument("cursor is invalid"))?;
    let decoded = String::from_utf8(bytes)
        .map_err(|_| ApplicationError::invalid_argument("cursor is invalid"))?;
    let (updated_at, request_id) = decoded
        .split_once('\0')
        .ok_or_else(|| ApplicationError::invalid_argument("cursor is invalid"))?;
    if updated_at.is_empty() || request_id.contains('\0') {
        return Err(ApplicationError::invalid_argument("cursor is invalid"));
    }
    let request_id = crate::feedback::canonical_uuid(request_id, "cursor")?;
    Ok(ListCursor {
        updated_at: updated_at.to_owned(),
        request_id,
    })
}

pub(crate) fn validate_file_name(value: &str) -> Result<String, ApplicationError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 255 {
        return Err(ApplicationError::invalid_argument(
            "file_name must contain 1 to 255 characters",
        ));
    }
    if value.contains(['/', '\\', '\0']) || value == "." || value == ".." {
        return Err(ApplicationError::invalid_argument(
            "file_name must be a plain file name",
        ));
    }
    Ok(value.to_owned())
}

pub(crate) fn detect_image_media_type(contents: &[u8]) -> Option<&'static str> {
    if contents.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if contents.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if contents.starts_with(b"GIF87a") || contents.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if contents.len() >= 12
        && contents.starts_with(b"RIFF")
        && contents.get(8..12) == Some(b"WEBP")
    {
        Some("image/webp")
    } else {
        None
    }
}

pub(crate) fn normalize_image_file_name(file_name: &str, media_type: &str) -> String {
    let allowed_extensions: &[&str] = match media_type {
        "image/png" => &["png"],
        "image/jpeg" => &["jpg", "jpeg", "jfif"],
        "image/gif" => &["gif"],
        "image/webp" => &["webp"],
        _ => &[],
    };
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    if extension
        .as_deref()
        .is_some_and(|extension| allowed_extensions.contains(&extension))
    {
        return file_name.to_owned();
    }
    let stem = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .filter(|stem| !stem.is_empty())
        .unwrap_or(file_name);
    format!("{stem}.{}", allowed_extensions.first().unwrap_or(&"image"))
}
