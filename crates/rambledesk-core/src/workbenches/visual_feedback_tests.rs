use super::*;

fn data() -> VisualFeedbackData {
    VisualFeedbackData {
        title: "Canvas".into(),
        source_version: "v1".into(),
        width: 640,
        height: 400,
        background_color: None,
        image_file_name: None,
    }
}
fn annotation() -> VisualFeedbackAnnotation {
    VisualFeedbackAnnotation {
        id: "note-1".into(),
        kind: VisualFeedbackAnnotationKind::Arrow,
        points: vec![
            VisualFeedbackPoint { x: 10, y: 20 },
            VisualFeedbackPoint { x: 50, y: 60 },
        ],
        color: "#ff0000".into(),
        stroke_width: 4,
        text: String::new(),
        body: "Move this control".into(),
    }
}
fn spec() -> WorkbenchSpec {
    WorkbenchSpec {
        kind: "visual_feedback".into(),
        version: 1,
        data: WorkbenchData::VisualFeedback(data()),
    }
}
fn state(annotations: Vec<VisualFeedbackAnnotation>, id: Option<&str>) -> WorkbenchState {
    WorkbenchState::VisualFeedback(VisualFeedbackState {
        annotations,
        composite_attachment_id: id.map(str::to_owned),
    })
}
#[test]
fn annotations_only_and_notes_only_publish_with_a_composite_and_cancel_omits_results() {
    let spec = spec();
    for (annotations, notes) in [(vec![annotation()], ""), (vec![], "Overall feedback")] {
        let draft = FeedbackDraft {
            workbench_state: Some(state(annotations, Some("composite-1"))),
        };
        let package = prepare_feedback_submission(Some(&spec), Some(&draft), notes)
            .unwrap()
            .unwrap();
        assert!(complete(&data(), package.result.as_ref()));
        assert_eq!(workbench_package(&spec, Some(&draft), false).result, None);
        let restored: WorkbenchPackage = serde_value::to_value(&package)
            .unwrap()
            .deserialize_into()
            .unwrap();
        assert_eq!(restored, package);
    }
    let empty = FeedbackDraft {
        workbench_state: Some(state(vec![], Some("composite-1"))),
    };
    assert_eq!(
        prepare_feedback_submission(Some(&spec), Some(&empty), "").unwrap_err(),
        WorkbenchSubmissionError::Empty
    );
    assert_eq!(
        prepare_feedback_submission(Some(&spec), None, "Notes").unwrap_err(),
        WorkbenchSubmissionError::Incomplete
    );
}
#[test]
fn drafts_may_await_composite_or_text_but_cannot_publish_unfinished_content() {
    let mut text = annotation();
    text.kind = VisualFeedbackAnnotationKind::Text;
    text.points.truncate(1);
    let draft = state(vec![text], None);
    assert!(draft_valid(&data(), Some(&draft)));
    assert!(has_input(&data(), Some(&draft)));
    assert!(result(&data(), Some(&draft)).is_none());
    let WorkbenchState::VisualFeedback(mut draft) = draft else {
        unreachable!()
    };
    draft.composite_attachment_id = Some("composite".into());
    assert!(
        result(
            &data(),
            Some(&WorkbenchState::VisualFeedback(draft.clone()))
        )
        .is_none()
    );
    draft.annotations[0].text = "A label".into();
    assert!(result(&data(), Some(&WorkbenchState::VisualFeedback(draft))).is_some());
}
#[test]
fn validates_shapes_coordinates_colors_uniqueness_and_resource_limits() {
    let mut invalid = vec![];
    let mut a = annotation();
    a.points[0].x = 641;
    invalid.push(vec![a]);
    let mut a = annotation();
    a.color = "red".into();
    invalid.push(vec![a]);
    let mut a = annotation();
    a.color = "#ff00🙂".into();
    invalid.push(vec![a]);
    let mut a = annotation();
    a.stroke_width = 0;
    invalid.push(vec![a]);
    let mut a = annotation();
    a.points[1] = a.points[0];
    invalid.push(vec![a]);
    let mut a = annotation();
    a.kind = VisualFeedbackAnnotationKind::Rectangle;
    a.points[1].x = a.points[0].x;
    invalid.push(vec![a]);
    let mut a = annotation();
    a.kind = VisualFeedbackAnnotationKind::Freehand;
    a.points = vec![a.points[0]; 2049];
    invalid.push(vec![a]);
    let mut a = annotation();
    a.text = "Unexpected".into();
    invalid.push(vec![a]);
    let mut a = annotation();
    a.body = "x".repeat(4001);
    invalid.push(vec![a]);
    invalid.push(vec![annotation(), annotation()]);
    invalid.push(
        (0..501)
            .map(|n| {
                let mut a = annotation();
                a.id = format!("a-{n}");
                a
            })
            .collect(),
    );
    invalid.push(
        (0..10)
            .map(|n| {
                let mut a = annotation();
                a.id = format!("a-{n}");
                a.kind = VisualFeedbackAnnotationKind::Freehand;
                a.points = vec![a.points[0]; 2048];
                a
            })
            .collect(),
    );
    for annotations in invalid {
        let state = state(annotations, Some("composite"));
        assert!(!draft_valid(&data(), Some(&state)));
        assert!(result(&data(), Some(&state)).is_none());
    }
    let mut invalid = data();
    invalid.background_color = Some("transparent".into());
    assert!(validate(&invalid).is_err());
    invalid.background_color = Some("#ABCDEF".into());
    assert!(validate(&invalid).is_ok());
    invalid.width = 4097;
    assert!(validate(&invalid).is_err());
    assert!(visual_feedback_image_dimensions(b"\x89PNG\r\n\x1a\nopaque", "image/png").is_err());
}
#[test]
fn discovery_and_missing_ambiguous_or_wrong_image_material_are_rejected() {
    let description = describe().unwrap();
    validate_workbench(&description.example).unwrap();
    assert!(!definition().approval);
    assert_eq!(
        description.result_schema.as_value()["properties"]["composite_attachment_id"]["type"],
        "string"
    );
    let mut spec = spec();
    let WorkbenchData::VisualFeedback(data) = &mut spec.data else {
        unreachable!()
    };
    data.image_file_name = Some("material.png".into());
    assert!(validate_visual_feedback_material(Some(&spec), &[]).is_err());
    let mut png = std::io::Cursor::new(vec![]);
    image::DynamicImage::new_rgb8(640, 400)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let attachment = crate::NewRequestAttachment {
        attachment_id: "input".into(),
        file_name: "material.png".into(),
        media_type: "image/png".into(),
        contents: png.into_inner(),
        sha256: "hash".into(),
    };
    assert!(
        validate_visual_feedback_material(Some(&spec), std::slice::from_ref(&attachment)).is_ok()
    );
    assert!(
        validate_visual_feedback_material(Some(&spec), &[attachment.clone(), attachment.clone()])
            .is_err()
    );
    let mut wrong_dimensions = attachment.clone();
    let mut png = std::io::Cursor::new(vec![]);
    image::DynamicImage::new_rgb8(32, 32)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    wrong_dimensions.contents = png.into_inner();
    assert!(validate_visual_feedback_material(Some(&spec), &[wrong_dimensions]).is_err());
    let mut wrong = attachment;
    wrong.contents = b"opaque".to_vec();
    assert!(validate_visual_feedback_material(Some(&spec), &[wrong]).is_err());
}

#[test]
fn image_dimensions_follow_exif_display_orientation() {
    use image::ImageEncoder;
    for orientation in 1..=8 {
        // One little-endian TIFF IFD entry: Orientation (SHORT), followed by no next IFD.
        let mut exif = b"II*\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
        exif.extend_from_slice(&[orientation, 0, 0, 0, 0, 0, 0, 0]);
        let mut jpeg = vec![];
        let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut jpeg);
        encoder.set_exif_metadata(exif).unwrap();
        encoder
            .write_image(&[0; 32 * 64 * 3], 32, 64, image::ExtendedColorType::Rgb8)
            .unwrap();
        let expected = if orientation >= 5 { (64, 32) } else { (32, 64) };
        assert_eq!(
            visual_feedback_image_dimensions(&jpeg, "image/jpeg").unwrap(),
            expected,
            "EXIF orientation {orientation}"
        );
    }
}
