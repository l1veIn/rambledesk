use rambledesk_core::kernel::ports::FactStoreError;
use rambledesk_core::kernel::{ArtifactRole, PackagePurpose, PackageRecord};
use serde::Serialize;

#[derive(Serialize)]
struct Manifest<'a> {
    schema_version: u32,
    package_id: &'a str,
    submission_id: &'a str,
    package_purpose: &'static str,
    request_id: Option<&'a str>,
    content_digest: &'a str,
    artifacts: Vec<ManifestArtifact<'a>>,
    published_at: &'a str,
}

#[derive(Serialize)]
struct ManifestArtifact<'a> {
    artifact_id: &'a str,
    role: &'a str,
    position: u32,
    display_name: &'a str,
    media_type: &'a str,
    size_bytes: u64,
    sha256: &'a str,
}

/// Single Storage-owned Package Manifest projection. It deliberately omits
/// Artifact Store keys and never recalculates either Core-owned digest.
pub(super) fn build_manifest(package: &PackageRecord) -> Result<String, FactStoreError> {
    serde_json::to_string(&Manifest {
        schema_version: package.schema_version,
        package_id: package.package_id.as_str(),
        submission_id: package.submission_id.as_str(),
        package_purpose: purpose_label(package.purpose),
        request_id: package.request_id.as_ref().map(|value| value.as_str()),
        content_digest: &package.content_digest,
        artifacts: package
            .artifacts
            .iter()
            .map(|artifact| ManifestArtifact {
                artifact_id: artifact.artifact_id.as_str(),
                role: artifact.role.digest_label(),
                position: artifact.position,
                display_name: &artifact.display_name,
                media_type: &artifact.media_type,
                size_bytes: artifact.size_bytes,
                sha256: &artifact.sha256,
            })
            .collect(),
        published_at: &package.published_at,
    })
    .map_err(|_| FactStoreError::Storage)
}

pub(super) fn purpose_label(value: PackagePurpose) -> &'static str {
    match value {
        PackagePurpose::Launch => "launch",
        PackagePurpose::Response => "response",
    }
}

pub(super) fn role_from_label(value: String) -> ArtifactRole {
    match value.as_str() {
        "feedback" => ArtifactRole::Feedback,
        "uncooked" => ArtifactRole::Uncooked,
        "attachment" => ArtifactRole::Attachment,
        _ => ArtifactRole::Other(value),
    }
}
