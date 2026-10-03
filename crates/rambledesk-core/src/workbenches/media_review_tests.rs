use super::*;
fn data() -> MediaReviewData {
    MediaReviewData {
        title: "Clip".into(),
        source_version: "v1".into(),
        media_kind: MediaReviewKind::Video,
        media_file_name: "clip.mp4".into(),
        duration_ms: 8000,
    }
}
fn state(start: u32, end: Option<u32>, body: &str) -> WorkbenchState {
    WorkbenchState::MediaReview(MediaReviewState {
        comments: vec![MediaReviewComment {
            id: "c-1".into(),
            start_ms: start,
            end_ms: end,
            body: body.into(),
        }],
    })
}
#[test]
fn immutable_duration_and_comment_ranges_are_strict_and_drafts_may_be_empty() {
    let mut data = data();
    assert!(validate(&data).is_ok());
    data.duration_ms = 0;
    assert!(validate(&data).is_err());
    data.duration_ms = 86400001;
    assert!(validate(&data).is_err());
    data.duration_ms = 8000;
    for name in ["../clip.mp4", "https://a/clip.mp4", " clip.mp4 "] {
        data.media_file_name = name.into();
        assert!(validate(&data).is_err());
    }
    data.media_file_name = "clip.mp4".into();
    for (start, end) in [
        (8001, None),
        (8000, Some(8001)),
        (100, Some(100)),
        (100, Some(99)),
    ] {
        assert!(!draft_valid(&data, Some(&state(start, end, "comment"))));
    }
    assert!(result(&data, Some(&state(8000, None, "End point"))).is_some());
    assert!(draft_valid(&data, Some(&state(0, Some(8000), ""))));
    assert!(result(&data, Some(&state(0, Some(8000), ""))).is_none());
    assert!(result(&data, Some(&state(0, None, "\u{85}"))).is_none());
    assert!(result(&data, Some(&state(0, None, "\u{feff}"))).is_some());
    assert!(complete(&data, result(&data, None).as_ref()));
    assert!(!has_input(&data, None));
}
#[test]
fn material_requires_exact_unique_file_name_magic_bytes_and_matching_kind() {
    let spec = WorkbenchSpec {
        kind: "media_review".into(),
        version: 1,
        data: WorkbenchData::MediaReview(data()),
    };
    let attachment = crate::NewRequestAttachment {
        attachment_id: "id".into(),
        file_name: "clip.mp4".into(),
        media_type: "video/mp4".into(),
        contents: b"\0\0\0\x18ftypisom\0\0\0\0isommp42".to_vec(),
        sha256: "".into(),
    };
    assert!(validate_media_review_material(Some(&spec), &[]).is_err());
    assert!(validate_media_review_material(Some(&spec), std::slice::from_ref(&attachment)).is_ok());
    assert!(
        validate_media_review_material(Some(&spec), &[attachment.clone(), attachment.clone()])
            .is_err()
    );
    let mut wrong = attachment.clone();
    wrong.media_type = "audio/mp4".into();
    assert!(validate_media_review_material(Some(&spec), &[wrong]).is_err());
    let mut wrong = attachment;
    wrong.contents = b"not a media file".to_vec();
    assert!(validate_media_review_material(Some(&spec), &[wrong]).is_err());
}
