use serde::{Deserialize, Serialize};

use super::ArtifactId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactInput {
    pub display_name: String,
    pub media_type: String,
    pub contents: Vec<u8>,
}

/// Result of storing bytes in an Artifact Store.
///
/// `sha256` is always `sha256:` followed by 64 lowercase hexadecimal digits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredBlob {
    pub storage_key: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactRole {
    Feedback,
    Uncooked,
    Attachment,
    Other(String),
}

impl ArtifactRole {
    pub fn digest_label(&self) -> &str {
        match self {
            Self::Feedback => "feedback",
            Self::Uncooked => "uncooked",
            Self::Attachment => "attachment",
            Self::Other(value) => value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageArtifact {
    pub artifact_id: ArtifactId,
    pub role: ArtifactRole,
    pub position: u32,
    pub display_name: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub sha256: String,
    /// Opaque Artifact Store key. It is never a Package path contract.
    pub storage_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestArtifact {
    pub artifact_id: ArtifactId,
    pub position: u32,
    pub display_name: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub storage_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmissionArtifact {
    pub artifact_id: ArtifactId,
    pub position: u32,
    pub display_name: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub storage_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftArtifact {
    pub artifact_id: ArtifactId,
    pub position: u32,
    pub display_name: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub storage_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveredArtifact {
    pub artifact_id: ArtifactId,
    pub role: ArtifactRole,
    pub position: u32,
    pub display_name: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub contents: Vec<u8>,
}
