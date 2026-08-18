use std::path::{Path, PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};

use crate::workspace::validation::{
    detect_image_media_type, normalize_image_file_name, validate_file_name,
};

use super::{ApplicationError, RequestAttachmentInput};

pub(super) fn load_request_attachment(
    attachment: &RequestAttachmentInput,
) -> Result<(String, Vec<u8>, String), ApplicationError> {
    let file_name = validate_file_name(&attachment.file_name)?;
    match (
        attachment.markdown.as_deref(),
        attachment.contents_base64.as_deref(),
        attachment.path.as_deref(),
    ) {
        (Some(markdown), None, None) => load_inline_markdown(file_name, markdown),
        (None, Some(encoded), None) => load_inline_image(file_name, encoded),
        (None, None, Some(path)) => load_path_attachment(file_name, path),
        _ => Err(ApplicationError::invalid_argument(
            "each attachment must provide exactly one of markdown, contents_base64, or path",
        )),
    }
}

fn load_inline_markdown(
    file_name: String,
    markdown: &str,
) -> Result<(String, Vec<u8>, String), ApplicationError> {
    ensure_markdown_file_name(&file_name)?;
    Ok((
        file_name,
        markdown.as_bytes().to_vec(),
        "text/markdown".to_owned(),
    ))
}

fn load_inline_image(
    file_name: String,
    encoded: &str,
) -> Result<(String, Vec<u8>, String), ApplicationError> {
    let contents = BASE64_STANDARD.decode(encoded).map_err(|_| {
        ApplicationError::invalid_argument(
            "attachment contents_base64 must be valid standard base64",
        )
    })?;
    classify_image(file_name, contents)
}

fn load_path_attachment(
    file_name: String,
    raw_path: &str,
) -> Result<(String, Vec<u8>, String), ApplicationError> {
    let path = parse_absolute_path(raw_path)?;
    let metadata = std::fs::metadata(&path).map_err(|_| {
        ApplicationError::invalid_argument(format!(
            "attachment path does not exist: {}",
            path.display()
        ))
    })?;
    if !metadata.is_file() {
        return Err(ApplicationError::invalid_argument(
            "attachment path must be a regular file",
        ));
    }
    if metadata.len() > crate::MAX_ATTACHMENT_BYTES as u64 {
        return Err(ApplicationError::invalid_argument(format!(
            "attachment exceeds the {} MiB limit",
            crate::MAX_ATTACHMENT_BYTES / 1024 / 1024
        )));
    }
    let contents = std::fs::read(&path).map_err(|error| {
        ApplicationError::invalid_argument(format!(
            "failed to read attachment path {}: {error}",
            path.display()
        ))
    })?;
    if is_markdown_file_name(&file_name) {
        return Ok((file_name, contents, "text/markdown".to_owned()));
    }
    classify_image(file_name, contents)
}

fn classify_image(
    file_name: String,
    contents: Vec<u8>,
) -> Result<(String, Vec<u8>, String), ApplicationError> {
    let media_type = detect_image_media_type(&contents).ok_or_else(|| {
        ApplicationError::invalid_argument(
            "path and base64 attachments must be PNG, JPEG, GIF, or WebP images unless file_name ends with .md or .markdown",
        )
    })?;
    Ok((
        normalize_image_file_name(&file_name, media_type),
        contents,
        media_type.to_owned(),
    ))
}

fn parse_absolute_path(raw: &str) -> Result<PathBuf, ApplicationError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ApplicationError::invalid_argument(
            "attachment path cannot be empty",
        ));
    }
    let path = Path::new(trimmed);
    if !path.is_absolute() {
        return Err(ApplicationError::invalid_argument(
            "attachment path must be an absolute filesystem path",
        ));
    }
    Ok(path.to_path_buf())
}

fn ensure_markdown_file_name(file_name: &str) -> Result<(), ApplicationError> {
    if is_markdown_file_name(file_name) {
        Ok(())
    } else {
        Err(ApplicationError::invalid_argument(
            "markdown attachments must use a .md or .markdown file name",
        ))
    }
}

fn is_markdown_file_name(file_name: &str) -> bool {
    matches!(
        file_name
            .rsplit_once('.')
            .map(|(_, extension)| extension.to_ascii_lowercase())
            .as_deref(),
        Some("md" | "markdown")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const TINY_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x08, 0xd7, 0x63, 0xf8,
        0xff, 0x1f, 0x00, 0x01, 0x05, 0x01, 0x01, 0x5a, 0xdd, 0xdb, 0x3d, 0x00, 0x00, 0x00, 0x00,
        0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    fn attachment(
        file_name: &str,
        markdown: Option<&str>,
        contents_base64: Option<&str>,
        path: Option<&str>,
    ) -> RequestAttachmentInput {
        RequestAttachmentInput {
            file_name: file_name.to_owned(),
            markdown: markdown.map(ToOwned::to_owned),
            contents_base64: contents_base64.map(ToOwned::to_owned),
            path: path.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn reads_an_image_from_an_absolute_path() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("shot.png");
        std::fs::write(&path, TINY_PNG).expect("write png");
        let (file_name, contents, media_type) = load_request_attachment(&attachment(
            "shot.png",
            None,
            None,
            Some(path.to_str().unwrap()),
        ))
        .expect("load path");
        assert_eq!(file_name, "shot.png");
        assert_eq!(media_type, "image/png");
        assert_eq!(contents, TINY_PNG);
    }

    #[test]
    fn reads_markdown_from_an_absolute_path() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("notes.md");
        std::fs::write(&path, "# Hello\n").expect("write markdown");
        let (file_name, contents, media_type) = load_request_attachment(&attachment(
            "notes.md",
            None,
            None,
            Some(path.to_str().unwrap()),
        ))
        .expect("load markdown path");
        assert_eq!(file_name, "notes.md");
        assert_eq!(media_type, "text/markdown");
        assert_eq!(contents, b"# Hello\n");
    }

    #[test]
    fn rejects_relative_paths_and_mixed_sources() {
        assert!(
            load_request_attachment(&attachment("shot.png", None, None, Some("shot.png"))).is_err()
        );
        assert!(
            load_request_attachment(&attachment(
                "shot.png",
                None,
                Some("aaaa"),
                Some("/tmp/shot.png"),
            ))
            .is_err()
        );
    }

    #[test]
    fn rejects_missing_files() {
        let missing = std::env::temp_dir().join("rambledesk-missing-attachment.png");
        let _ = std::fs::remove_file(&missing);
        assert!(
            load_request_attachment(&attachment(
                "shot.png",
                None,
                None,
                Some(missing.to_str().unwrap()),
            ))
            .is_err()
        );
    }
}
