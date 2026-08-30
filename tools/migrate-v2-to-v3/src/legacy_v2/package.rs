use std::path::{Component, Path, PathBuf};

use rambledesk_core::kernel::{MAX_ARTIFACT_BYTES, MAX_ARTIFACT_TOTAL_BYTES};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_MANIFEST_ARTIFACTS: usize = 128;

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
    PackageDirectoryUnreadable,
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
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    file_name: Option<String>,
    #[serde(default)]
    media_type: Option<String>,
    path: String,
    byte_size: u64,
    sha256: String,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyPackageArtifact {
    pub id: String,
    pub display_name: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub legacy_path: String,
    pub metadata_synthesized: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyPackage {
    pub manifest: LegacyPackageArtifact,
    pub feedback: LegacyPackageArtifact,
    pub uncooked: Option<LegacyPackageArtifact>,
    pub attachments: Vec<LegacyPackageArtifact>,
    pub request_attachments: Vec<LegacyPackageArtifact>,
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
    let manifest_bytes =
        read_safe_file_limited(&canonical_directory, &manifest_path, MAX_MANIFEST_BYTES)
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
        || manifest.attachments.len() + manifest.request_attachments.len() > MAX_MANIFEST_ARTIFACTS
    {
        return Err(LegacyPackageIssue::ManifestInvalid);
    }

    let feedback = read_safe_file(&canonical_directory, &feedback_path)
        .await
        .map_err(|_| LegacyPackageIssue::FeedbackUnreadable)?;
    if sha256_hex(&feedback) != manifest.feedback_sha256 {
        return Err(LegacyPackageIssue::FeedbackDigestMismatch);
    }
    let mut total_bytes = feedback.len();
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
            total_bytes = total_bytes
                .checked_add(uncooked.len())
                .ok_or(LegacyPackageIssue::AttachmentInvalid)?;
        }
        _ => return Err(LegacyPackageIssue::UncookedInvalid),
    }
    let declared_attachment_bytes = manifest
        .attachments
        .iter()
        .chain(&manifest.request_attachments)
        .try_fold(0usize, |total, attachment| {
            total.checked_add(attachment.byte_size as usize)
        })
        .ok_or(LegacyPackageIssue::AttachmentInvalid)?;
    if total_bytes
        .checked_add(declared_attachment_bytes)
        .is_none_or(|total| total > MAX_ARTIFACT_TOTAL_BYTES)
    {
        return Err(LegacyPackageIssue::AttachmentInvalid);
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

pub(crate) async fn package_directory_contains(
    paths: &LegacyPackagePaths,
    forbidden: &Path,
) -> bool {
    let directory = Path::new(&paths.directory_path);
    let Ok(canonical_directory) = safe_directory(directory).await else {
        return false;
    };
    let Ok(canonical_forbidden) = tokio::fs::canonicalize(forbidden).await else {
        return false;
    };
    canonical_forbidden.starts_with(canonical_directory)
}

pub(crate) async fn read_package(
    request_id: &str,
    paths: &LegacyPackagePaths,
) -> Result<LegacyPackage, LegacyPackageIssue> {
    inspect_package(request_id, paths).await?;
    let directory = PathBuf::from(&paths.directory_path);
    let canonical_directory = safe_directory(&directory).await?;
    let manifest_path = PathBuf::from(&paths.manifest_path);
    let manifest_bytes =
        read_safe_file_limited(&canonical_directory, &manifest_path, MAX_MANIFEST_BYTES)
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
        || manifest.attachments.len() + manifest.request_attachments.len() > MAX_MANIFEST_ARTIFACTS
    {
        return Err(LegacyPackageIssue::ManifestInvalid);
    }
    let feedback_bytes = read_safe_file(&canonical_directory, &PathBuf::from(&paths.markdown_path))
        .await
        .map_err(|_| LegacyPackageIssue::FeedbackUnreadable)?;
    if sha256_hex(&feedback_bytes) != manifest.feedback_sha256 {
        return Err(LegacyPackageIssue::FeedbackDigestMismatch);
    }
    let feedback = LegacyPackageArtifact {
        id: format!("legacy-{request_id}-feedback"),
        display_name: "feedback.md".to_owned(),
        media_type: "text/markdown; charset=utf-8".to_owned(),
        bytes: feedback_bytes,
        sha256: format!("sha256:{}", manifest.feedback_sha256),
        legacy_path: paths.markdown_path.clone(),
        metadata_synthesized: false,
    };
    let uncooked = match (&manifest.uncooked_markdown, &manifest.uncooked_sha256) {
        (Some(relative), Some(digest))
            if relative == "uncooked.md" && valid_legacy_digest(digest) =>
        {
            let bytes = read_safe_file(&canonical_directory, &directory.join(relative))
                .await
                .map_err(|_| LegacyPackageIssue::UncookedInvalid)?;
            if sha256_hex(&bytes) != *digest {
                return Err(LegacyPackageIssue::UncookedInvalid);
            }
            Some(LegacyPackageArtifact {
                id: format!("legacy-{request_id}-uncooked"),
                display_name: "uncooked.md".to_owned(),
                media_type: "text/markdown; charset=utf-8".to_owned(),
                bytes,
                sha256: format!("sha256:{digest}"),
                legacy_path: directory.join(relative).to_string_lossy().into_owned(),
                metadata_synthesized: false,
            })
        }
        (None, None) => None,
        _ => return Err(LegacyPackageIssue::UncookedInvalid),
    };
    let declared_attachment_bytes = manifest
        .attachments
        .iter()
        .chain(&manifest.request_attachments)
        .try_fold(0usize, |total, attachment| {
            total.checked_add(attachment.byte_size as usize)
        })
        .ok_or(LegacyPackageIssue::AttachmentInvalid)?;
    let base_bytes =
        feedback.bytes.len() + uncooked.as_ref().map_or(0, |artifact| artifact.bytes.len());
    if base_bytes
        .checked_add(declared_attachment_bytes)
        .is_none_or(|total| total > MAX_ARTIFACT_TOTAL_BYTES)
    {
        return Err(LegacyPackageIssue::AttachmentInvalid);
    }
    let attachments = read_attachment_group(
        &canonical_directory,
        &directory,
        manifest.attachments,
        "attachments",
        request_id,
    )
    .await?;
    let request_attachments = read_attachment_group(
        &canonical_directory,
        &directory,
        manifest.request_attachments,
        "request-attachments",
        request_id,
    )
    .await?;
    Ok(LegacyPackage {
        manifest: LegacyPackageArtifact {
            id: format!("legacy-{request_id}-manifest"),
            display_name: "manifest.json".to_owned(),
            media_type: "application/json".to_owned(),
            bytes: manifest_bytes.clone(),
            sha256: format!("sha256:{}", paths.manifest_sha256),
            legacy_path: paths.manifest_path.clone(),
            metadata_synthesized: false,
        },
        feedback,
        uncooked,
        attachments,
        request_attachments,
    })
}

async fn read_attachment_group(
    canonical_directory: &Path,
    directory: &Path,
    attachments: Vec<LegacyAttachment>,
    expected_parent: &str,
    request_id: &str,
) -> Result<Vec<LegacyPackageArtifact>, LegacyPackageIssue> {
    let mut result = Vec::with_capacity(attachments.len());
    for (position, attachment) in attachments.into_iter().enumerate() {
        let bytes = read_safe_file(canonical_directory, &directory.join(&attachment.path))
            .await
            .map_err(|_| LegacyPackageIssue::AttachmentInvalid)?;
        if bytes.len() as u64 != attachment.byte_size || sha256_hex(&bytes) != attachment.sha256 {
            return Err(LegacyPackageIssue::AttachmentInvalid);
        }
        let path = Path::new(&attachment.path);
        let fallback_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("attachment.bin");
        let metadata_synthesized = attachment
            .file_name
            .as_ref()
            .is_none_or(|value| value.trim().is_empty() || value.contains(['/', '\\']))
            || attachment
                .media_type
                .as_ref()
                .is_none_or(|value| value.trim().is_empty());
        let display_name = attachment
            .file_name
            .filter(|value| !value.trim().is_empty() && !value.contains(['/', '\\']))
            .unwrap_or_else(|| fallback_name.to_owned());
        let media_type = attachment
            .media_type
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| infer_media_type(&display_name));
        result.push(LegacyPackageArtifact {
            id: attachment
                .id
                .unwrap_or_else(|| format!("legacy-{request_id}-{expected_parent}-{position}")),
            display_name,
            media_type,
            bytes,
            sha256: format!("sha256:{}", attachment.sha256),
            legacy_path: directory
                .join(&attachment.path)
                .to_string_lossy()
                .into_owned(),
            metadata_synthesized,
        });
    }
    Ok(result)
}

fn infer_media_type(name: &str) -> String {
    match Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("md") => "text/markdown".to_owned(),
        Some("png") => "image/png".to_owned(),
        Some("jpg" | "jpeg") => "image/jpeg".to_owned(),
        Some("webp") => "image/webp".to_owned(),
        _ => "application/octet-stream".to_owned(),
    }
}

async fn inspect_attachment(
    canonical_directory: &Path,
    directory: &Path,
    attachment: &LegacyAttachment,
    expected_parent: &str,
) -> Result<(), LegacyPackageIssue> {
    if !valid_relative_attachment_path(&attachment.path, expected_parent)
        || !valid_legacy_digest(&attachment.sha256)
        || attachment.byte_size > MAX_ARTIFACT_BYTES as u64
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
        .map_err(|_| LegacyPackageIssue::PackageDirectoryUnreadable)?;
    if metadata.file_type().is_symlink() {
        return Err(LegacyPackageIssue::UnsafePackageDirectory);
    }
    if !metadata.is_dir() {
        return Err(LegacyPackageIssue::PackageDirectoryUnreadable);
    }
    tokio::fs::canonicalize(path)
        .await
        .map_err(|_| LegacyPackageIssue::PackageDirectoryUnreadable)
}

async fn read_safe_file(canonical_directory: &Path, path: &Path) -> Result<Vec<u8>, ()> {
    read_safe_file_limited(canonical_directory, path, MAX_ARTIFACT_BYTES).await
}

async fn read_safe_file_limited(
    canonical_directory: &Path,
    path: &Path,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ()> {
    let metadata = tokio::fs::symlink_metadata(path).await.map_err(|_| ())?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > maximum_bytes as u64
    {
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
