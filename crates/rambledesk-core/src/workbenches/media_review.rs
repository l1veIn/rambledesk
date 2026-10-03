use super::*;
use std::collections::HashSet;
#[cfg(test)]
#[path = "media_review_tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum MediaReviewKind {
    Audio,
    Video,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct MediaReviewData {
    #[schemars(length(min = 1, max = 200))]
    pub title: String,
    #[schemars(length(min = 1, max = 128))]
    pub source_version: String,
    pub media_kind: MediaReviewKind,
    /// Exact unique file_name of a frozen request attachment, at most 20 MiB.
    pub media_file_name: String,
    /// Declared immutable timebase; the player reports mismatched or unsupported material.
    #[schemars(range(min = 1, max = 86400000))]
    pub duration_ms: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct MediaReviewComment {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,63}$"))]
    pub id: String,
    pub start_ms: u32,
    /// Null for a time point; otherwise strictly later than start_ms, at most duration_ms.
    pub end_ms: Option<u32>,
    /// Markdown with shared attachment references; drafts may be empty.
    #[schemars(length(max = 4000))]
    pub body: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct MediaReviewState {
    #[schemars(length(max = 500))]
    pub comments: Vec<MediaReviewComment>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct MediaReviewResult {
    pub source_version: String,
    pub duration_ms: u32,
    pub comments: Vec<MediaReviewComment>,
}
pub(super) fn definition() -> WorkbenchDefinition {
    let mut definition = WorkbenchDefinition::new(
        WorkbenchSummary {
            kind: "media_review",
            version: 1,
            name: "Media review / 媒体评审",
            purpose: "Play an immutable short audio/video attachment and leave comments at times or ranges.",
            returns: "Source version, declared millisecond timebase and timestamped comments, with optional shared notes",
            interaction: "media_review",
        },
        describe,
    );
    definition.strict_state = true;
    definition.notes_only = true;
    definition
}
fn describe() -> Result<WorkbenchDescription, ApplicationError> {
    Ok(WorkbenchDescription {
        summary: definition().summary,
        input_schema: schemars::schema_for!(MediaReviewData),
        result_schema: schemars::schema_for!(MediaReviewResult),
        example: WorkbenchSpec {
            kind: "media_review".into(),
            version: 1,
            data: WorkbenchData::MediaReview(MediaReviewData {
                title: "Review the short walkthrough".into(),
                source_version: "walkthrough-v1".into(),
                media_kind: MediaReviewKind::Video,
                media_file_name: "walkthrough.mp4".into(),
                duration_ms: 8000,
            }),
        },
        instructions: "Provide a fixed request audio/video attachment and reference its exact unique file_name. Maximum 20 MiB per attachment; no network URLs or direct player filesystem paths. Audio formats: WAV, MP3, M4A, Ogg/Opus, WebM audio (.weba); video: MP4, WebM, Ogg video (.ogv). Containers are checked by magic bytes and filename; browser codec support remains required. Supply an immutable declared duration_ms from 1 to 86400000. The player reports playback failure or a materially different actual duration without changing anchors. Comments have unique stable IDs, integer start_ms from 0 to duration_ms, and end_ms null for points or start_ms < end_ms <= duration_ms for ranges. At most 500 comments; bodies are Markdown up to 4000 Unicode scalars with shared attachment references. Empty drafts may be saved but published comments require visible text. Submit timed comments, shared notes or both. No autoplay, transcoding, editing or approval verdict is supported.",
    })
}
pub(super) fn validate(data: &MediaReviewData) -> Result<(), ApplicationError> {
    super::validation::text("media_review.title", &data.title, 200)?;
    super::validation::text("media_review.source_version", &data.source_version, 128)?;
    if crate::workspace::validation::validate_file_name(&data.media_file_name)?
        != data.media_file_name
        || !(1..=86400000).contains(&data.duration_ms)
    {
        return Err(ApplicationError::invalid_argument(
            "media_review requires a plain exact filename and duration_ms from 1 to 86400000",
        ));
    }
    Ok(())
}
fn comments_valid(
    data: &MediaReviewData,
    comments: &[MediaReviewComment],
    published: bool,
) -> bool {
    if comments.len() > 500 {
        return false;
    }
    let mut ids = HashSet::new();
    comments.iter().all(|comment| {
        super::validation::valid_item_id(&comment.id)
            && ids.insert(&comment.id)
            && comment.start_ms <= data.duration_ms
            && comment
                .end_ms
                .is_none_or(|end| end > comment.start_ms && end <= data.duration_ms)
            && !comment.body.contains('\0')
            && comment.body.chars().count() <= 4000
            && (!published || !comment.body.trim().is_empty())
    })
}
pub(super) fn draft_valid(data: &MediaReviewData, state: Option<&WorkbenchState>) -> bool {
    match state {
        None => true,
        Some(WorkbenchState::MediaReview(state)) => comments_valid(data, &state.comments, false),
        _ => false,
    }
}
pub(super) fn result(
    data: &MediaReviewData,
    state: Option<&WorkbenchState>,
) -> Option<WorkbenchResult> {
    let comments = match state {
        None => vec![],
        Some(WorkbenchState::MediaReview(state)) if comments_valid(data, &state.comments, true) => {
            state.comments.clone()
        }
        _ => return None,
    };
    Some(WorkbenchResult::MediaReview(MediaReviewResult {
        source_version: data.source_version.clone(),
        duration_ms: data.duration_ms,
        comments,
    }))
}
pub(super) fn has_input(data: &MediaReviewData, state: Option<&WorkbenchState>) -> bool {
    matches!(state, Some(WorkbenchState::MediaReview(state)) if !state.comments.is_empty() && comments_valid(data, &state.comments, false))
}
pub(super) fn complete(data: &MediaReviewData, result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::MediaReview(result)) if result.source_version == data.source_version && result.duration_ms == data.duration_ms && comments_valid(data, &result.comments, true))
}
pub(super) fn result_has_input(result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::MediaReview(result)) if !result.comments.is_empty())
}
pub(super) fn legacy_actions(_: &MediaReviewData) -> Vec<ActionInput> {
    vec![]
}

pub fn validate_media_review_material(
    spec: Option<&WorkbenchSpec>,
    attachments: &[crate::NewRequestAttachment],
) -> Result<(), ApplicationError> {
    let Some(WorkbenchSpec {
        data: WorkbenchData::MediaReview(data),
        ..
    }) = spec
    else {
        return Ok(());
    };
    let matches: Vec<_> = attachments
        .iter()
        .filter(|item| item.file_name == data.media_file_name)
        .collect();
    let [attachment] = matches.as_slice() else {
        return Err(ApplicationError::invalid_argument(
            "media_review requires one exact unique media_file_name request attachment",
        ));
    };
    let kind = match data.media_kind {
        MediaReviewKind::Audio => "audio/",
        MediaReviewKind::Video => "video/",
    };
    let actual = crate::workspace::media::detect_playable_media_type(
        &attachment.contents,
        &attachment.file_name,
    );
    if actual != Some(attachment.media_type.as_str()) || !attachment.media_type.starts_with(kind) {
        return Err(ApplicationError::invalid_argument(
            "media_review material bytes, filename and media_kind must match a supported audio/video container",
        ));
    }
    Ok(())
}
