use super::{
    ArtifactId, ArtifactInput, ArtifactRole, Core, CoreError, CoreErrorCode, PackageArtifact,
    RequestArtifact, StoredBlob, SubmissionArtifact,
    core_support::{GeneratedId, package_artifact, submission_artifact},
    digest::bytes_digest,
    ports::PutArtifact,
};

impl Core {
    pub(super) async fn stage_launch_package_artifacts(
        &self,
        body_markdown: &str,
        inputs: &[ArtifactInput],
    ) -> Result<(Vec<PackageArtifact>, Vec<SubmissionArtifact>), CoreError> {
        let mut package = Vec::with_capacity(inputs.len() + 2);
        package.push(
            self.stage_package_artifact(
                ArtifactRole::Feedback,
                0,
                "feedback.md",
                "text/markdown; charset=utf-8",
                body_markdown.as_bytes(),
            )
            .await?,
        );
        package.push(
            self.stage_package_artifact(
                ArtifactRole::Uncooked,
                1,
                "uncooked.md",
                "text/markdown; charset=utf-8",
                body_markdown.as_bytes(),
            )
            .await?,
        );
        let mut submission = Vec::with_capacity(inputs.len());
        for (position, input) in inputs.iter().enumerate() {
            let blob = self.put_blob(&input.contents).await?;
            package.push(package_artifact(
                ArtifactId::new_id(),
                ArtifactRole::Attachment,
                (position + 2) as u32,
                input,
                &blob,
            ));
            submission.push(submission_artifact(
                ArtifactId::new_id(),
                position as u32,
                input,
                blob,
            ));
        }
        Ok((package, submission))
    }

    pub(super) async fn stage_response_package_artifacts(
        &self,
        feedback_markdown: &str,
        uncooked_markdown: &str,
        inputs: &[ArtifactInput],
    ) -> Result<(Vec<PackageArtifact>, Vec<SubmissionArtifact>), CoreError> {
        let mut package = Vec::with_capacity(inputs.len() + 2);
        package.push(
            self.stage_package_artifact(
                ArtifactRole::Feedback,
                0,
                "feedback.md",
                "text/markdown; charset=utf-8",
                feedback_markdown.as_bytes(),
            )
            .await?,
        );
        package.push(
            self.stage_package_artifact(
                ArtifactRole::Uncooked,
                1,
                "uncooked.md",
                "text/markdown; charset=utf-8",
                uncooked_markdown.as_bytes(),
            )
            .await?,
        );
        let mut submission = Vec::with_capacity(inputs.len());
        for (position, input) in inputs.iter().enumerate() {
            let blob = self.put_blob(&input.contents).await?;
            package.push(package_artifact(
                ArtifactId::new_id(),
                ArtifactRole::Attachment,
                (position + 2) as u32,
                input,
                &blob,
            ));
            submission.push(submission_artifact(
                ArtifactId::new_id(),
                position as u32,
                input,
                blob,
            ));
        }
        Ok((package, submission))
    }

    async fn stage_package_artifact(
        &self,
        role: ArtifactRole,
        position: u32,
        display_name: &str,
        media_type: &str,
        contents: &[u8],
    ) -> Result<PackageArtifact, CoreError> {
        let blob = self.put_blob(contents).await?;
        Ok(PackageArtifact {
            artifact_id: ArtifactId::new_id(),
            role,
            position,
            display_name: display_name.to_owned(),
            media_type: media_type.to_owned(),
            size_bytes: blob.size_bytes,
            sha256: blob.sha256,
            storage_key: blob.storage_key,
        })
    }

    pub(super) async fn stage_submission_artifacts(
        &self,
        inputs: &[ArtifactInput],
    ) -> Result<Vec<SubmissionArtifact>, CoreError> {
        let mut artifacts = Vec::with_capacity(inputs.len());
        for (position, input) in inputs.iter().enumerate() {
            let blob = self.put_blob(&input.contents).await?;
            artifacts.push(submission_artifact(
                ArtifactId::new_id(),
                position as u32,
                input,
                blob,
            ));
        }
        Ok(artifacts)
    }

    pub(super) async fn stage_request_artifacts(
        &self,
        inputs: &[ArtifactInput],
    ) -> Result<Vec<RequestArtifact>, CoreError> {
        let mut artifacts = Vec::with_capacity(inputs.len());
        for (position, input) in inputs.iter().enumerate() {
            let blob = self.put_blob(&input.contents).await?;
            artifacts.push(RequestArtifact {
                artifact_id: ArtifactId::new_id(),
                position: position as u32,
                display_name: input.display_name.clone(),
                media_type: input.media_type.clone(),
                size_bytes: blob.size_bytes,
                sha256: blob.sha256,
                storage_key: blob.storage_key,
            });
        }
        Ok(artifacts)
    }

    pub(super) async fn put_blob(&self, contents: &[u8]) -> Result<StoredBlob, CoreError> {
        let expected_sha256 = bytes_digest(contents);
        let stored = self
            .artifacts
            .put(PutArtifact {
                contents: contents.to_vec(),
                expected_sha256: expected_sha256.clone(),
            })
            .await?;
        if stored.storage_key.is_empty()
            || stored.sha256 != expected_sha256
            || stored.size_bytes != contents.len() as u64
        {
            return Err(CoreError::new(
                CoreErrorCode::ArtifactDigestMismatch,
                "Artifact Store returned metadata that does not match the input bytes",
                false,
            ));
        }
        Ok(stored)
    }
}
