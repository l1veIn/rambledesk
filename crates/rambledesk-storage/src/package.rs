use std::path::{Path, PathBuf};

use crate::SqliteFeedbackStore;
use async_trait::async_trait;
use rambledesk_core::{
    FeedbackPackagePublisher, FeedbackResultView, PublishedFeedbackPackage, RepositoryError,
    SubmissionPlan,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

#[async_trait]
impl FeedbackPackagePublisher for SqliteFeedbackStore {
    async fn publish(
        &self,
        plan: &SubmissionPlan,
    ) -> Result<PublishedFeedbackPackage, RepositoryError> {
        let _guard = self.publish_lock.lock().await;
        publish_package(plan).await
    }
}

async fn publish_package(
    plan: &SubmissionPlan,
) -> Result<PublishedFeedbackPackage, RepositoryError> {
    let final_directory = PathBuf::from(&plan.directory_path);
    let parent = final_directory
        .parent()
        .ok_or(RepositoryError::PackagePublish)?;
    validate_publication_parent(plan, parent).await?;

    let markdown = render_markdown(plan);
    let feedback_sha256 = hex::encode(Sha256::digest(markdown.as_bytes()));
    let manifest = render_manifest(plan, &feedback_sha256)?;
    let manifest_sha256 = hex::encode(Sha256::digest(manifest.as_bytes()));

    if tokio::fs::try_exists(&final_directory)
        .await
        .map_err(package_error)?
    {
        validate_existing_package(plan, &manifest).await?;
        sync_directory(parent).await?;
        return Ok(published_result(plan, manifest_sha256));
    }

    let temporary = PathBuf::from(&plan.temp_directory_path);
    if temporary.parent() != Some(parent) || temporary == final_directory {
        return Err(RepositoryError::PackagePublish);
    }
    if tokio::fs::try_exists(&temporary)
        .await
        .map_err(package_error)?
    {
        tokio::fs::remove_dir_all(&temporary)
            .await
            .map_err(package_error)?;
    }
    tokio::fs::create_dir(&temporary)
        .await
        .map_err(package_error)?;
    let write_result = async {
        tokio::fs::create_dir(temporary.join("attachments"))
            .await
            .map_err(package_error)?;
        write_synced(&temporary.join("feedback.md"), markdown.as_bytes()).await?;
        write_synced(&temporary.join("manifest.json"), manifest.as_bytes()).await?;
        sync_directory(&temporary).await?;
        validate_publication_parent(plan, parent).await?;
        tokio::fs::rename(&temporary, &final_directory)
            .await
            .map_err(package_error)?;
        validate_publication_parent(plan, parent).await?;
        sync_directory(parent).await?;
        validate_publication_parent(plan, parent).await
    }
    .await;

    if let Err(error) = write_result {
        validate_publication_parent(plan, parent).await?;
        if tokio::fs::try_exists(&final_directory)
            .await
            .map_err(package_error)?
        {
            if tokio::fs::try_exists(&temporary).await.unwrap_or(false) {
                let _ = tokio::fs::remove_dir_all(&temporary).await;
            }
            validate_existing_package(plan, &manifest).await?;
            sync_directory(parent).await?;
        } else {
            if tokio::fs::try_exists(&temporary).await.unwrap_or(false) {
                let _ = tokio::fs::remove_dir_all(&temporary).await;
            }
            return Err(error);
        }
    }

    Ok(published_result(plan, manifest_sha256))
}

async fn validate_publication_parent(
    plan: &SubmissionPlan,
    parent: &Path,
) -> Result<(), RepositoryError> {
    let final_directory = Path::new(&plan.directory_path);
    let temporary = Path::new(&plan.temp_directory_path);
    if final_directory.parent() != Some(parent)
        || temporary.parent() != Some(parent)
        || final_directory == temporary
    {
        return Err(RepositoryError::PackagePublish);
    }
    let metadata = tokio::fs::symlink_metadata(parent)
        .await
        .map_err(package_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(RepositoryError::PackagePublish);
    }
    let canonical = tokio::fs::canonicalize(parent)
        .await
        .map_err(package_error)?;
    if canonical != parent {
        return Err(RepositoryError::PackagePublish);
    }
    Ok(())
}

fn render_markdown(plan: &SubmissionPlan) -> String {
    let mut markdown = format!(
        "# RambleDesk Feedback\n\n## Request\n\n{}\n\n## Actions\n\n",
        plan.what_happened
    );
    for action in &plan.actions {
        markdown.push_str(&format!("- **{}**: {}\n", action.id, action.instruction));
    }
    markdown.push_str("\n## Operator Feedback\n\n");
    markdown.push_str(&plan.body_markdown);
    if !markdown.ends_with('\n') {
        markdown.push('\n');
    }
    markdown
}

fn render_manifest(
    plan: &SubmissionPlan,
    feedback_sha256: &str,
) -> Result<String, RepositoryError> {
    let value = json!({
        "schema_version": 1,
        "request_id": plan.request_id,
        "project_id": plan.project_id,
        "agent": plan.agent,
        "session_id": plan.session_id,
        "submitted_at": plan.submitted_at,
        "source_revision": plan.source_revision,
        "draft_revision": plan.source_revision,
        "feedback_markdown": "feedback.md",
        "feedback_sha256": feedback_sha256,
        "attachments": [],
    });
    let mut rendered = serde_json::to_string_pretty(&value).map_err(package_error)?;
    rendered.push('\n');
    Ok(rendered)
}

async fn validate_existing_package(
    plan: &SubmissionPlan,
    expected_manifest: &str,
) -> Result<(), RepositoryError> {
    let manifest = tokio::fs::read_to_string(&plan.manifest_path)
        .await
        .map_err(package_error)?;
    if manifest != expected_manifest {
        return Err(RepositoryError::PackagePublish);
    }
    if !tokio::fs::try_exists(&plan.markdown_path)
        .await
        .map_err(package_error)?
    {
        return Err(RepositoryError::PackagePublish);
    }
    let markdown = tokio::fs::read_to_string(&plan.markdown_path)
        .await
        .map_err(package_error)?;
    if markdown != render_markdown(plan) {
        return Err(RepositoryError::PackagePublish);
    }
    Ok(())
}

async fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), RepositoryError> {
    let mut file = tokio::fs::File::create(path).await.map_err(package_error)?;
    file.write_all(bytes).await.map_err(package_error)?;
    file.flush().await.map_err(package_error)?;
    file.sync_all().await.map_err(package_error)
}

#[cfg(unix)]
async fn sync_directory(path: &Path) -> Result<(), RepositoryError> {
    tokio::fs::File::open(path)
        .await
        .map_err(package_error)?
        .sync_all()
        .await
        .map_err(package_error)
}

#[cfg(not(unix))]
async fn sync_directory(_path: &Path) -> Result<(), RepositoryError> {
    Err(RepositoryError::PackagePublish)
}

fn published_result(plan: &SubmissionPlan, manifest_sha256: String) -> PublishedFeedbackPackage {
    PublishedFeedbackPackage {
        result: FeedbackResultView {
            package_uri: plan.package_uri.clone(),
            directory_path: plan.directory_path.clone(),
            markdown_path: plan.markdown_path.clone(),
            manifest_path: plan.manifest_path.clone(),
        },
        manifest_sha256,
        published_at: plan.submitted_at.clone(),
    }
}

fn package_error<T>(_error: T) -> RepositoryError {
    RepositoryError::PackagePublish
}
