use base64::{Engine as _, engine::general_purpose::STANDARD};
use rambledesk_core::{FeedbackPackageContent, WorkbenchData, WorkbenchResult};
use rmcp::model::ContentBlock;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

const INLINE_PNG_BYTES: u64 = 2 * 1024 * 1024;

/// Inline only this immutable result's referenced PNG, never an arbitrary attachment or stale image.
pub(super) async fn composite_preview(package: &FeedbackPackageContent) -> Option<ContentBlock> {
    let workbench = package.manifest.workbench.as_ref()?;
    if workbench.input.kind != "visual_feedback" || workbench.input.version != 1 {
        return None;
    }
    let (WorkbenchData::VisualFeedback(data), Some(WorkbenchResult::VisualFeedback(result))) =
        (&workbench.input.data, &workbench.result)
    else {
        return None;
    };
    let unavailable = |reason: &str| {
        ContentBlock::text(format!(
            "Composite PNG preview is unavailable: {reason}. Read the exact composite_attachment_id in the published package manifest and its attachment path; structured annotations and package paths are still returned."
        ))
    };
    let matches: Vec<_> = package
        .manifest
        .attachments
        .iter()
        .enumerate()
        .filter(|(_, attachment)| attachment.id == result.composite_attachment_id)
        .collect();
    if matches.len() != 1 || package.attachment_paths.len() != package.manifest.attachments.len() {
        return Some(unavailable(
            "the result does not match exactly one manifest feedback attachment",
        ));
    }
    let (index, attachment) = matches[0];
    if attachment.media_type != "image/png" {
        return Some(unavailable("the referenced attachment is not PNG"));
    }
    if attachment.byte_size == 0 || attachment.byte_size > INLINE_PNG_BYTES {
        return Some(unavailable("the PNG exceeds the 2 MiB inline image budget"));
    }
    let path = &package.attachment_paths[index];
    let Ok(file) = tokio::fs::File::open(path).await else {
        return Some(unavailable("the PNG file cannot be read"));
    };
    let Ok(metadata) = file.metadata().await else {
        return Some(unavailable("the PNG file cannot be read"));
    };
    if !metadata.is_file() || metadata.len() != attachment.byte_size {
        return Some(unavailable(
            "the PNG file size no longer matches its manifest",
        ));
    }
    let mut bytes = Vec::new();
    if file
        .take(INLINE_PNG_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .is_err()
    {
        return Some(unavailable("the PNG file cannot be read"));
    }
    if bytes.len() as u64 != attachment.byte_size
        || hex::encode(Sha256::digest(&bytes)) != attachment.sha256
    {
        return Some(unavailable(
            "the PNG bytes no longer match their immutable manifest",
        ));
    }
    let expected = (data.width, data.height);
    let checked = tokio::task::spawn_blocking(move || {
        (rambledesk_core::visual_feedback_image_dimensions(&bytes, "image/png").ok()
            == Some(expected))
        .then(|| STANDARD.encode(bytes))
    })
    .await
    .ok()
    .flatten();
    Some(match checked {
        Some(encoded) => ContentBlock::image(encoded, "image/png"),
        None => unavailable("the PNG cannot be decoded at the declared canvas dimensions"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rambledesk_core::{
        FeedbackPackageAttachment, FeedbackPackageManifest, VisualFeedbackData,
        VisualFeedbackResult, WorkbenchPackage, WorkbenchSpec,
    };

    fn png() -> Vec<u8> {
        let mut bytes = std::io::Cursor::new(vec![]);
        image::DynamicImage::new_rgb8(32, 32)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }
    async fn fixture(path: &std::path::Path) -> FeedbackPackageContent {
        let bytes = png();
        tokio::fs::write(path, &bytes).await.unwrap();
        let manifest: FeedbackPackageManifest = serde_json::from_value(serde_json::json!({
            "schema_version":1,"request_id":"request","title":"Canvas","host_id":"test","host_session_id":"session","source_hint":null,
            "submitted_at":"2026-10-02","source_revision":2,"draft_revision":2,"feedback_markdown":"feedback.md","feedback_sha256":"hash","attachments":[]
        })).unwrap();
        FeedbackPackageContent {
            manifest: FeedbackPackageManifest {
                workbench: Some(WorkbenchPackage {
                    input: WorkbenchSpec {
                        kind: "visual_feedback".into(),
                        version: 1,
                        data: WorkbenchData::VisualFeedback(VisualFeedbackData {
                            title: "Canvas".into(),
                            source_version: "v1".into(),
                            width: 32,
                            height: 32,
                            background_color: None,
                            image_file_name: None,
                        }),
                    },
                    result: Some(WorkbenchResult::VisualFeedback(VisualFeedbackResult {
                        source_version: "v1".into(),
                        width: 32,
                        height: 32,
                        annotations: vec![],
                        composite_attachment_id: "composite".into(),
                    })),
                }),
                attachments: vec![FeedbackPackageAttachment {
                    id: "composite".into(),
                    file_name: "canvas.png".into(),
                    media_type: "image/png".into(),
                    byte_size: bytes.len() as u64,
                    sha256: hex::encode(Sha256::digest(&bytes)),
                    path: "attachments/canvas.png".into(),
                }],
                ..manifest
            },
            markdown: "Notes".into(),
            uncooked_markdown: None,
            attachment_paths: vec![path.to_string_lossy().into()],
            request_attachment_paths: vec![],
        }
    }
    #[tokio::test]
    async fn inlines_only_the_exact_manifest_png_and_preserves_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let package = fixture(&directory.path().join("canvas.png")).await;
        let preview = composite_preview(&package).await.unwrap();
        let image = preview.as_image().unwrap();
        assert_eq!(image.mime_type, "image/png");
        assert_eq!(STANDARD.decode(&image.data).unwrap(), png());
    }
    #[tokio::test]
    async fn dangling_non_png_corrupt_wrong_size_and_over_budget_results_are_explicit() {
        let directory = tempfile::tempdir().unwrap();
        let package = fixture(&directory.path().join("canvas.png")).await;
        let mut invalid = package.clone();
        invalid.manifest.attachments[0].id = "other".into();
        assert!(
            composite_preview(&invalid)
                .await
                .unwrap()
                .as_image()
                .is_none()
        );
        let mut invalid = package.clone();
        invalid.manifest.attachments[0].media_type = "image/jpeg".into();
        assert!(
            composite_preview(&invalid)
                .await
                .unwrap()
                .as_image()
                .is_none()
        );
        let mut invalid = package.clone();
        invalid.manifest.attachments[0].byte_size = INLINE_PNG_BYTES + 1;
        assert!(
            composite_preview(&invalid)
                .await
                .unwrap()
                .as_text()
                .unwrap()
                .text
                .contains("2 MiB")
        );
        let mut invalid = package.clone();
        if let WorkbenchData::VisualFeedback(data) =
            &mut invalid.manifest.workbench.as_mut().unwrap().input.data
        {
            data.width = 64
        }
        assert!(
            composite_preview(&invalid)
                .await
                .unwrap()
                .as_image()
                .is_none()
        );
        tokio::fs::write(&package.attachment_paths[0], b"opaque bytes")
            .await
            .unwrap();
        assert!(
            composite_preview(&package)
                .await
                .unwrap()
                .as_image()
                .is_none()
        );
    }
}
