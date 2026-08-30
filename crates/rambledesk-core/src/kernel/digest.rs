use sha2::{Digest, Sha256};

use super::{
    AccessMode, AcpSessionLinkId, ArtifactInput, ContextReference, FeedbackAction,
    FeedbackSubmission, LaunchConfiguration, LaunchSubmission, PackageArtifact, PackageId,
    PackagePurpose, RambleContent, RequestId, SteeringSubmission, SubmissionId,
};

const DIGEST_VERSION: &str = "rambledesk-kernel-digest-v1";

struct CanonicalDigest(Sha256);

impl CanonicalDigest {
    fn new(kind: &str) -> Self {
        let mut value = Self(Sha256::new());
        value.field("schema", DIGEST_VERSION.as_bytes());
        value.field("kind", kind.as_bytes());
        value
    }

    fn field(&mut self, label: &str, value: &[u8]) {
        self.0.update((label.len() as u64).to_be_bytes());
        self.0.update(label.as_bytes());
        self.0.update((value.len() as u64).to_be_bytes());
        self.0.update(value);
    }

    fn optional(&mut self, label: &str, value: Option<&str>) {
        match value {
            Some(value) => {
                self.field(&format!("{label}.present"), b"1");
                self.field(label, value.as_bytes());
            }
            None => self.field(&format!("{label}.present"), b"0"),
        }
    }

    fn number(&mut self, label: &str, value: u64) {
        self.field(label, &value.to_be_bytes());
    }

    fn finish(self) -> String {
        format!("sha256:{}", hex::encode(self.0.finalize()))
    }
}

pub(super) fn bytes_digest(contents: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(contents)))
}

fn add_launch_configuration(digest: &mut CanonicalDigest, config: &LaunchConfiguration) {
    digest.field("agent_profile_id", config.agent_profile_id.as_bytes());
    digest.field("launch_profile_id", config.launch_profile_id.as_bytes());
    digest.field("workspace_reference", config.workspace_reference.as_bytes());
    digest.optional("model", config.model.as_deref());
    digest.optional("reasoning_effort", config.reasoning_effort.as_deref());
    digest.field(
        "access_mode",
        match config.access_mode {
            AccessMode::ReadOnly => b"read_only",
            AccessMode::WorkspaceWrite => b"workspace_write",
            AccessMode::Yolo => b"yolo",
        },
    );
    digest.field("agent_config_json", config.agent_config_json.as_bytes());
}

fn add_artifact_inputs(digest: &mut CanonicalDigest, artifacts: &[ArtifactInput]) {
    digest.number("artifact_count", artifacts.len() as u64);
    for (position, artifact) in artifacts.iter().enumerate() {
        digest.number("artifact.position", position as u64);
        digest.field("artifact.display_name", artifact.display_name.as_bytes());
        digest.field("artifact.media_type", artifact.media_type.as_bytes());
        digest.field(
            "artifact.content_sha256",
            bytes_digest(&artifact.contents).as_bytes(),
        );
        digest.number("artifact.size_bytes", artifact.contents.len() as u64);
    }
}

fn add_ramble(digest: &mut CanonicalDigest, ramble: &RambleContent) {
    digest.field("document_json", ramble.document_json.as_bytes());
    digest.field("body_markdown", ramble.body_markdown.as_bytes());
    add_artifact_inputs(digest, &ramble.artifacts);
}

pub(super) fn launch_submission_digest(input: &LaunchSubmission) -> String {
    let mut digest = CanonicalDigest::new("launch_submission");
    digest.field("title", input.title.as_bytes());
    add_launch_configuration(&mut digest, &input.launch_configuration);
    add_ramble(&mut digest, &input.ramble);
    digest.finish()
}

pub(super) fn steering_submission_digest(input: &SteeringSubmission) -> String {
    let mut digest = CanonicalDigest::new("steering_submission");
    digest.field("session_id", input.session_id.as_str().as_bytes());
    add_ramble(&mut digest, &input.ramble);
    digest.finish()
}

pub(super) fn feedback_submission_digest(input: &FeedbackSubmission) -> String {
    let mut digest = CanonicalDigest::new("feedback_submission");
    digest.field("request_id", input.request_id.as_str().as_bytes());
    digest.number("expected_draft_revision", input.expected_draft_revision);
    digest.field("document_json", input.document_json.as_bytes());
    digest.field("uncooked_markdown", input.uncooked_markdown.as_bytes());
    digest.field("feedback_markdown", input.feedback_markdown.as_bytes());
    digest.optional("cooking_model", input.cooking_model.as_deref());
    add_artifact_inputs(&mut digest, &input.artifacts);
    digest.finish()
}

pub(super) fn feedback_request_digest(
    session_id: &str,
    source_link_id: Option<&AcpSessionLinkId>,
    title: &str,
    instructions: &str,
    actions: &[FeedbackAction],
    context_refs: &[ContextReference],
    artifacts: &[ArtifactInput],
) -> String {
    let mut digest = CanonicalDigest::new("feedback_request");
    digest.field("session_id", session_id.as_bytes());
    digest.optional(
        "source_link_id",
        source_link_id.map(AcpSessionLinkId::as_str),
    );
    digest.field("title", title.as_bytes());
    digest.field("instructions", instructions.as_bytes());
    digest.number("action_count", actions.len() as u64);
    for action in actions {
        digest.field("action.id", action.id.as_bytes());
        digest.field("action.instruction", action.instruction.as_bytes());
    }
    digest.number("context_ref_count", context_refs.len() as u64);
    for context_ref in context_refs {
        digest.field("context_ref.label", context_ref.label.as_bytes());
        digest.field("context_ref.uri", context_ref.uri.as_bytes());
    }
    add_artifact_inputs(&mut digest, artifacts);
    digest.finish()
}

pub(super) fn package_content_digest(
    purpose: PackagePurpose,
    request_id: Option<&RequestId>,
    artifacts: &[PackageArtifact],
) -> String {
    let mut digest = CanonicalDigest::new("feedback_package_content");
    digest.field(
        "purpose",
        match purpose {
            PackagePurpose::Launch => b"launch",
            PackagePurpose::Response => b"response",
        },
    );
    digest.optional("request_id", request_id.map(RequestId::as_str));
    digest.number("artifact_count", artifacts.len() as u64);
    for artifact in artifacts {
        digest.number("artifact.position", artifact.position as u64);
        digest.field("artifact.role", artifact.role.digest_label().as_bytes());
        digest.field("artifact.display_name", artifact.display_name.as_bytes());
        digest.field("artifact.media_type", artifact.media_type.as_bytes());
        digest.number("artifact.size_bytes", artifact.size_bytes);
        digest.field("artifact.sha256", artifact.sha256.as_bytes());
        // Deliberately excludes artifact_id and storage_key. Moving content or
        // retrying publication cannot change what the Package is.
    }
    digest.finish()
}

pub(super) struct ManifestDigestInput<'a> {
    pub package_id: &'a PackageId,
    pub submission_id: &'a SubmissionId,
    pub purpose: PackagePurpose,
    pub request_id: Option<&'a RequestId>,
    pub content_digest: &'a str,
    pub schema_version: u32,
    pub artifacts: &'a [PackageArtifact],
    pub published_at: &'a str,
}

pub(super) fn package_manifest_digest(input: ManifestDigestInput<'_>) -> String {
    let mut digest = CanonicalDigest::new("feedback_package_manifest");
    digest.field("package_id", input.package_id.as_str().as_bytes());
    digest.field("submission_id", input.submission_id.as_str().as_bytes());
    digest.field(
        "purpose",
        match input.purpose {
            PackagePurpose::Launch => b"launch",
            PackagePurpose::Response => b"response",
        },
    );
    digest.optional("request_id", input.request_id.map(RequestId::as_str));
    digest.field("content_digest", input.content_digest.as_bytes());
    digest.number("schema_version", input.schema_version as u64);
    digest.number("artifact_count", input.artifacts.len() as u64);
    for artifact in input.artifacts {
        digest.field("artifact.id", artifact.artifact_id.as_str().as_bytes());
        digest.number("artifact.position", artifact.position as u64);
        digest.field("artifact.role", artifact.role.digest_label().as_bytes());
        digest.field("artifact.display_name", artifact.display_name.as_bytes());
        digest.field("artifact.media_type", artifact.media_type.as_bytes());
        digest.number("artifact.size_bytes", artifact.size_bytes);
        digest.field("artifact.sha256", artifact.sha256.as_bytes());
    }
    digest.field("published_at", input.published_at.as_bytes());
    digest.finish()
}

pub(super) fn agent_work_payload_digest(kind: &str, source_id: &str, payload: &str) -> String {
    let mut digest = CanonicalDigest::new("agent_work_payload");
    digest.field("work_kind", kind.as_bytes());
    digest.field("source_id", source_id.as_bytes());
    digest.field("payload", payload.as_bytes());
    digest.finish()
}
