//! Versioned request identity, kept separate from SQLite request mutations.
//!
//! Struct field order and optional-field inclusion below are persisted hash
//! contracts. Keep each historical serialization shape intact; changing a capture
//! projection or deduplicating the structs must not change an existing request identity.

use rambledesk_core::{ActionInput, ContextRef, NewFeedbackRequest, RepositoryError};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize)]
struct LegacyImmutableRequest<'a> {
    host_id: &'a str,
    host_session_id: &'a str,
    title: &'a str,
    what_happened: &'a str,
    actions: &'a [ActionInput],
    context_refs: &'a [ContextRef],
    source_hint: Option<&'a str>,
}

#[derive(Serialize)]
struct ImmutableRequest<'a> {
    host_id: &'a str,
    host_session_id: &'a str,
    title: &'a str,
    what_happened: &'a str,
    actions: &'a [ActionInput],
    context_refs: &'a [ContextRef],
    source_hint: Option<&'a str>,
    allow_finish: bool,
    final_summary: Option<&'a str>,
}

#[derive(Serialize)]
struct ImmutableRequestAttachment<'a> {
    file_name: &'a str,
    media_type: &'a str,
    byte_size: usize,
    sha256: &'a str,
}

#[derive(Serialize)]
struct ImmutableRequestWithAttachments<'a> {
    host_id: &'a str,
    host_session_id: &'a str,
    title: &'a str,
    what_happened: &'a str,
    actions: &'a [ActionInput],
    context_refs: &'a [ContextRef],
    attachments: Vec<ImmutableRequestAttachment<'a>>,
    source_hint: Option<&'a str>,
    allow_finish: bool,
    final_summary: Option<&'a str>,
}

pub(super) fn immutable_input_hash(
    request: &NewFeedbackRequest,
) -> Result<String, RepositoryError> {
    if let Some(spec) = &request.workbench
        && rambledesk_core::WorkbenchKind::resolve(&spec.kind, spec.version)
            .is_some_and(|kind| !kind.uses_legacy_action_identity())
    {
        // New contracts exclude capture projections from request identity. Keep
        // the original three contracts on their byte-for-byte legacy path below.
        #[derive(Serialize)]
        struct WorkbenchIdentity<'a> {
            identity_version: u32,
            host_id: &'a str,
            host_session_id: &'a str,
            title: &'a str,
            what_happened: &'a str,
            workbench: &'a rambledesk_core::WorkbenchSpec,
            context_refs: &'a [ContextRef],
            attachments: Vec<ImmutableRequestAttachment<'a>>,
            source_hint: Option<&'a str>,
            allow_finish: bool,
            final_summary: Option<&'a str>,
        }
        let bytes = serde_json::to_vec(&WorkbenchIdentity {
            identity_version: 1,
            host_id: &request.host_id,
            host_session_id: &request.host_session_id,
            title: &request.title,
            what_happened: &request.what_happened,
            workbench: spec,
            context_refs: &request.context_refs,
            attachments: request
                .attachments
                .iter()
                .map(|attachment| ImmutableRequestAttachment {
                    file_name: &attachment.file_name,
                    media_type: &attachment.media_type,
                    byte_size: attachment.contents.len(),
                    sha256: &attachment.sha256,
                })
                .collect(),
            source_hint: request.source_hint.as_deref(),
            allow_finish: request.allow_finish,
            final_summary: request.final_summary.as_deref(),
        })
        .map_err(|_| RepositoryError::Storage)?;
        return Ok(hex::encode(Sha256::digest(bytes)));
    }
    let bytes = if !request.attachments.is_empty() {
        serde_json::to_vec(&ImmutableRequestWithAttachments {
            host_id: &request.host_id,
            host_session_id: &request.host_session_id,
            title: &request.title,
            what_happened: &request.what_happened,
            actions: &request.actions,
            context_refs: &request.context_refs,
            attachments: request
                .attachments
                .iter()
                .map(|attachment| ImmutableRequestAttachment {
                    file_name: &attachment.file_name,
                    media_type: &attachment.media_type,
                    byte_size: attachment.contents.len(),
                    sha256: &attachment.sha256,
                })
                .collect(),
            source_hint: request.source_hint.as_deref(),
            allow_finish: request.allow_finish,
            final_summary: request.final_summary.as_deref(),
        })
    } else if request.allow_finish || request.final_summary.is_some() {
        serde_json::to_vec(&ImmutableRequest {
            host_id: &request.host_id,
            host_session_id: &request.host_session_id,
            title: &request.title,
            what_happened: &request.what_happened,
            actions: &request.actions,
            context_refs: &request.context_refs,
            source_hint: request.source_hint.as_deref(),
            allow_finish: request.allow_finish,
            final_summary: request.final_summary.as_deref(),
        })
    } else {
        // Preserve the original persisted hash for pre-final-approval requests.
        serde_json::to_vec(&LegacyImmutableRequest {
            host_id: &request.host_id,
            host_session_id: &request.host_session_id,
            title: &request.title,
            what_happened: &request.what_happened,
            actions: &request.actions,
            context_refs: &request.context_refs,
            source_hint: request.source_hint.as_deref(),
        })
    }
    .map_err(|_| RepositoryError::Storage)?;
    let bytes = match &request.workbench {
        Some(spec) => serde_json::to_vec(&(bytes, spec)).map_err(|_| RepositoryError::Storage)?,
        None => bytes,
    };
    Ok(hex::encode(Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(allow_finish: bool, final_summary: Option<&str>) -> NewFeedbackRequest {
        NewFeedbackRequest {
            workbench: None,
            request_id: "request-id".to_owned(),
            managed_session_id: None,
            host_session_record_id: "host-session-record-id".to_owned(),
            host_id: "generic".to_owned(),
            host_session_id: "session-1".to_owned(),
            title: "Review".to_owned(),
            what_happened: "Changed settings".to_owned(),
            actions: vec![ActionInput {
                id: "inspect".to_owned(),
                instruction: "Inspect settings".to_owned(),
            }],
            context_refs: vec![ContextRef {
                label: "diff".to_owned(),
                uri: "file:///tmp/change.diff".to_owned(),
            }],
            attachments: Vec::new(),
            source_hint: None,
            allow_finish,
            final_summary: final_summary.map(str::to_owned),
            created_at: "2026-08-03T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn immutable_hash_preserves_legacy_json_bytes() {
        assert_eq!(
            immutable_input_hash(&request(false, None)).expect("hash"),
            "53a9ef638f879a3d1790b8ef0d5ddf547405d569f5f29e6f33b32b93fe4f5b74"
        );
    }

    #[test]
    fn immutable_hash_covers_final_approval_fields() {
        assert_eq!(
            immutable_input_hash(&request(true, Some("Done"))).expect("hash"),
            "3abb4165d83f342b7a0f1f0f7218261c987a6abb19c4bf3b2f2c27b1c4ee212b"
        );
    }

    #[test]
    fn workbench_v1_hashes_preserve_the_original_persisted_contract() {
        // Goldens computed with the pre-refactor serialization and projections.
        // Do not update these when changing capture UI or validation internals.
        let fixtures = [
            (
                serde_json::json!({"type":"ramble","version":1,"data":{"actions":[{"id":"inspect","instruction":"Inspect settings"}]}}),
                "3d3458369227691912114c671cd9e50cadb9f04f3bfcac56009c7ecfd7e8f47b",
            ),
            (
                serde_json::json!({"type":"questions","version":1,"data":{"questions":[{"id":"audience","prompt":"Who?","label":"Audience","options":[{"value":"one","label":"One"},{"value":"two","label":"Two"}],"allowOther":true}]}}),
                "2e5d5a1140fedbb556035fd17af2b89e32afc641135cad27bd7da36cd4899320",
            ),
            (
                serde_json::json!({"type":"single_choice","version":1,"data":{"prompt":"Pick?","options":[{"id":"first","label":"First"},{"id":"second","label":"Second"}]}}),
                "ac171bbf3357a85c0d51950cafe197db2356aed22a49957328aa6612fb882256",
            ),
        ];
        for (wire, expected) in fixtures {
            let spec = serde_json::from_value(wire).unwrap();
            let mut request = request(false, None);
            request.actions = rambledesk_core::workbench_actions(&spec).unwrap();
            request.workbench = Some(spec);
            assert_eq!(immutable_input_hash(&request).unwrap(), expected);
        }
    }

    #[test]
    fn new_workbench_identity_excludes_derived_capture_actions() {
        let mut request = request(false, None);
        request.workbench = Some(
            rambledesk_core::describe_workbench(&rambledesk_core::DescribeWorkbenchInput {
                kind: "document_review".into(),
                version: Some(1),
            })
            .unwrap()
            .example,
        );
        let original = immutable_input_hash(&request).unwrap();
        request.actions.clear();
        assert_eq!(immutable_input_hash(&request).unwrap(), original);
        let rambledesk_core::WorkbenchData::DocumentReview(data) =
            &mut request.workbench.as_mut().unwrap().data
        else {
            unreachable!()
        };
        data.source_version.push_str("-edited");
        assert_ne!(immutable_input_hash(&request).unwrap(), original);
    }
}
