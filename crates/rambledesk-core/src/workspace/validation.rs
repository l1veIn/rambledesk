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

/// Detect a media type from attachment contents and file name.
///
/// Images are detected by magic bytes; PDF by its magic prefix. Other common
/// document types are mapped by extension. Unknown files fall back to
/// `application/octet-stream` so the user can still attach them.
pub(crate) fn detect_media_type(contents: &[u8], file_name: &str) -> &'static str {
    if let Some(image) = detect_image_media_type(contents) {
        return image;
    }
    if contents.starts_with(b"%PDF-") {
        return "application/pdf";
    }
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    match extension.as_deref() {
        Some("md") | Some("markdown") => "text/markdown",
        Some("txt") | Some("log") => "text/plain",
        Some("csv") => "text/csv",
        Some("json") => "application/json",
        Some("pdf") => "application/pdf",
        Some("doc") => "application/msword",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("xls") => "application/vnd.ms-excel",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("ppt") => "application/vnd.ms-powerpoint",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        Some("rtf") => "application/rtf",
        Some("zip") => "application/zip",
        Some("html") | Some("htm") => "text/html",
        Some("xml") => "application/xml",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_media_type_maps_images_and_docs() {
        assert_eq!(
            detect_media_type(b"\x89PNG\r\n\x1a\n\x00\x00\x00", "a.png"),
            "image/png"
        );
        assert_eq!(
            detect_media_type(b"%PDF-1.4\n", "report.pdf"),
            "application/pdf"
        );
        // PDF magic wins even when the extension does not match.
        assert_eq!(
            detect_media_type(b"%PDF-1.4\n", "renamed.txt"),
            "application/pdf"
        );
        assert_eq!(detect_media_type(b"# Title", "notes.md"), "text/markdown");
        assert_eq!(detect_media_type(b"x,y\n1,2", "data.csv"), "text/csv");
        assert_eq!(
            detect_media_type(b"PK\x03\x04\x00\x00", "plan.docx"),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        );
        assert_eq!(
            detect_media_type(b"anything", "unknown.zzz"),
            "application/octet-stream"
        );
        assert_eq!(
            detect_media_type(b"anything", "no-extension"),
            "application/octet-stream"
        );
    }
}
