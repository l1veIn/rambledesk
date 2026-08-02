use std::path::{Path, PathBuf};

use rambledesk_core::RepositoryError;

pub(super) struct PreparedPublicationPaths {
    pub(super) package_uri: String,
    pub(super) directory_path: String,
    pub(super) temp_directory_path: String,
    pub(super) markdown_path: String,
    pub(super) manifest_path: String,
}

pub(super) async fn prepare_publication_paths(
    request_id: &str,
    publication_id: &str,
    now: &str,
    library_root: &Path,
) -> Result<PreparedPublicationPaths, RepositoryError> {
    let feedback_root = prepare_app_feedback_root(library_root, publication_id).await?;
    let directory_name = format!("{}-{request_id}", compact_timestamp(now));
    let directory_path = feedback_root.join(directory_name);
    let temp_directory_path = feedback_root.join(format!(".{request_id}.tmp-{publication_id}"));
    let markdown_path = directory_path.join("feedback.md");
    let manifest_path = directory_path.join("manifest.json");
    Ok(PreparedPublicationPaths {
        package_uri: format!("rambledesk://feedback/{request_id}"),
        directory_path: path_string(&directory_path)?,
        temp_directory_path: path_string(&temp_directory_path)?,
        markdown_path: path_string(&markdown_path)?,
        manifest_path: path_string(&manifest_path)?,
    })
}

async fn prepare_app_feedback_root(
    library_root: &Path,
    publication_id: &str,
) -> Result<PathBuf, RepositoryError> {
    let fallback = library_root.join("feedback");
    tokio::fs::create_dir_all(&fallback)
        .await
        .map_err(storage_error)?;
    assert_not_symlink(&fallback).await?;
    let canonical_fallback = tokio::fs::canonicalize(&fallback)
        .await
        .map_err(package_error)?;
    verify_writable(&canonical_fallback, publication_id).await?;
    Ok(canonical_fallback)
}

async fn assert_not_symlink(path: &Path) -> Result<(), RepositoryError> {
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .map_err(package_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(RepositoryError::PackagePublish);
    }
    Ok(())
}

async fn verify_writable(directory: &Path, publication_id: &str) -> Result<(), RepositoryError> {
    let probe = directory.join(format!(".write-probe-{publication_id}"));
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    let file = options.open(&probe).await.map_err(package_error)?;
    file.sync_all().await.map_err(package_error)?;
    drop(file);
    tokio::fs::remove_file(&probe).await.map_err(package_error)
}

fn compact_timestamp(timestamp: &str) -> String {
    timestamp
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

pub(super) fn portable_file_name(file_name: &str) -> String {
    let mut value = file_name
        .chars()
        .map(|character| {
            if character.is_control() || "<>:\"/\\|?*".contains(character) {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    while value.ends_with([' ', '.']) {
        value.pop();
    }
    if value.is_empty() {
        "attachment".to_owned()
    } else {
        value
    }
}

pub(super) fn path_string(path: &Path) -> Result<String, RepositoryError> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or(RepositoryError::PackagePublish)
}

fn package_error<T>(_error: T) -> RepositoryError {
    RepositoryError::PackagePublish
}

fn storage_error<T>(_error: T) -> RepositoryError {
    RepositoryError::Storage
}
