use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub(crate) struct LegacyPackagePaths {
    pub directory_path: String,
    pub markdown_path: String,
    pub manifest_path: String,
    pub manifest_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyPackageIssue {
    MissingDatabaseResult,
    InvalidDatabasePath,
    UnsafePackageDirectory,
    ManifestUnreadable,
    ManifestDigestMismatch,
    ManifestInvalid,
    FeedbackUnreadable,
    FeedbackDigestMismatch,
    UncookedInvalid,
    AttachmentInvalid,
}

#[derive(Debug, Deserialize)]
struct LegacyManifest {
    schema_version: u32,
    request_id: String,
    feedback_markdown: String,
    feedback_sha256: String,
    uncooked_markdown: Option<String>,
    uncooked_sha256: Option<String>,
    #[serde(default)]
    attachments: Vec<LegacyAttachment>,
    #[serde(default)]
    request_attachments: Vec<LegacyAttachment>,
}

#[derive(Debug, Deserialize)]
struct LegacyAttachment {
    path: String,
    byte_size: u64,
    sha256: String,
}

pub(crate) async fn inspect_package(
    request_id: &str,
    paths: &LegacyPackagePaths,
) -> Result<(), LegacyPackageIssue> {
    let directory = PathBuf::from(&paths.directory_path);
    let feedback_path = PathBuf::from(&paths.markdown_path);
    let manifest_path = PathBuf::from(&paths.manifest_path);
    if !directory.is_absolute()
        || !feedback_path.is_absolute()
        || !manifest_path.is_absolute()
        || feedback_path.parent() != Some(directory.as_path())
        || feedback_path.file_name().and_then(|value| value.to_str()) != Some("feedback.md")
        || manifest_path.parent() != Some(directory.as_path())
        || manifest_path.file_name().and_then(|value| value.to_str()) != Some("manifest.json")
    {
        return Err(LegacyPackageIssue::InvalidDatabasePath);
    }
    let canonical_directory = safe_directory(&directory).await?;
    let manifest_bytes = read_safe_file(&canonical_directory, &manifest_path)
        .await
        .map_err(|_| LegacyPackageIssue::ManifestUnreadable)?;
    if sha256_hex(&manifest_bytes) != paths.manifest_sha256 {
        return Err(LegacyPackageIssue::ManifestDigestMismatch);
    }
    let manifest: LegacyManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| LegacyPackageIssue::ManifestInvalid)?;
    if manifest.schema_version != 1
        || manifest.request_id != request_id
        || manifest.feedback_markdown != "feedback.md"
        || !valid_legacy_digest(&manifest.feedback_sha256)
    {
        return Err(LegacyPackageIssue::ManifestInvalid);
    }

    let feedback = read_safe_file(&canonical_directory, &feedback_path)
        .await
        .map_err(|_| LegacyPackageIssue::FeedbackUnreadable)?;
    if sha256_hex(&feedback) != manifest.feedback_sha256 {
        return Err(LegacyPackageIssue::FeedbackDigestMismatch);
    }
    match (&manifest.uncooked_markdown, &manifest.uncooked_sha256) {
        (None, None) => {}
        (Some(relative), Some(expected))
            if relative == "uncooked.md" && valid_legacy_digest(expected) =>
        {
            let uncooked = read_safe_file(&canonical_directory, &directory.join(relative))
                .await
                .map_err(|_| LegacyPackageIssue::UncookedInvalid)?;
            if sha256_hex(&uncooked) != *expected {
                return Err(LegacyPackageIssue::UncookedInvalid);
            }
        }
        _ => return Err(LegacyPackageIssue::UncookedInvalid),
    }
    for attachment in &manifest.attachments {
        inspect_attachment(&canonical_directory, &directory, attachment, "attachments").await?;
    }
    for attachment in &manifest.request_attachments {
        inspect_attachment(
            &canonical_directory,
            &directory,
            attachment,
            "request-attachments",
        )
        .await?;
    }
    Ok(())
}

async fn inspect_attachment(
    canonical_directory: &Path,
    directory: &Path,
    attachment: &LegacyAttachment,
    expected_parent: &str,
) -> Result<(), LegacyPackageIssue> {
    if !valid_relative_attachment_path(&attachment.path, expected_parent)
        || !valid_legacy_digest(&attachment.sha256)
    {
        return Err(LegacyPackageIssue::AttachmentInvalid);
    }
    let bytes = read_safe_file(canonical_directory, &directory.join(&attachment.path))
        .await
        .map_err(|_| LegacyPackageIssue::AttachmentInvalid)?;
    if bytes.len() as u64 != attachment.byte_size || sha256_hex(&bytes) != attachment.sha256 {
        return Err(LegacyPackageIssue::AttachmentInvalid);
    }
    Ok(())
}

async fn safe_directory(path: &Path) -> Result<PathBuf, LegacyPackageIssue> {
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .map_err(|_| LegacyPackageIssue::UnsafePackageDirectory)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(LegacyPackageIssue::UnsafePackageDirectory);
    }
    tokio::fs::canonicalize(path)
        .await
        .map_err(|_| LegacyPackageIssue::UnsafePackageDirectory)
}

async fn read_safe_file(canonical_directory: &Path, path: &Path) -> Result<Vec<u8>, ()> {
    let metadata = tokio::fs::symlink_metadata(path).await.map_err(|_| ())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(());
    }
    let canonical = tokio::fs::canonicalize(path).await.map_err(|_| ())?;
    if !canonical.starts_with(canonical_directory) {
        return Err(());
    }
    tokio::fs::read(canonical).await.map_err(|_| ())
}

fn valid_relative_attachment_path(value: &str, expected_parent: &str) -> bool {
    let path = Path::new(value);
    if path.is_absolute() {
        return false;
    }
    let mut components = path.components();
    matches!(components.next(), Some(Component::Normal(parent)) if parent == expected_parent)
        && matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
}

fn valid_legacy_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_hex(contents: &[u8]) -> String {
    hex::encode(Sha256::digest(contents))
}
