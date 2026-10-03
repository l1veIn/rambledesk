use super::*;
use std::collections::HashSet;
#[cfg(test)]
#[path = "visual_feedback_tests.rs"]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
#[schemars(description = "Draw on an immutable request image or a blank canvas. The generated composite PNG must be at most 20 MiB; a compressed source image within 20 MiB does not guarantee that its PNG fits. Resize large photos before creating a new request.")]
pub struct VisualFeedbackData {
    #[schemars(length(min = 1, max = 200))]
    pub title: String,
    #[schemars(length(min = 1, max = 128))]
    pub source_version: String,
    #[schemars(range(min = 32, max = 4096))]
    pub width: u32,
    #[schemars(range(min = 32, max = 4096))]
    pub height: u32,
    /// Blank canvas background; defaults to white. An image is rendered over it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[schemars(regex(pattern = "^#[0-9a-fA-F]{6}$"))]
    pub background_color: Option<String>,
    /// Exact unique file_name of an immutable request image attachment; null for a blank canvas.
    pub image_file_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VisualFeedbackPoint {
    /// Integer canvas pixels, independent of display zoom.
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum VisualFeedbackAnnotationKind {
    Freehand,
    Arrow,
    Rectangle,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VisualFeedbackAnnotation {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,63}$"))]
    pub id: String,
    pub kind: VisualFeedbackAnnotationKind,
    /// Freehand: 2–2048 points; arrow/rectangle: two endpoints/corners; text: one origin.
    #[schemars(length(min = 1, max = 2048))]
    pub points: Vec<VisualFeedbackPoint>,
    #[schemars(regex(pattern = "^#[0-9a-fA-F]{6}$"))]
    pub color: String,
    /// Stroke width, or text font size, in integer canvas pixels.
    #[schemars(range(min = 1, max = 32))]
    pub stroke_width: u32,
    /// Nonblank for text annotations; empty for other shapes.
    #[schemars(length(max = 2000))]
    pub text: String,
    /// Optional opinion associated with the drawing.
    #[schemars(length(max = 4000))]
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VisualFeedbackState {
    #[schemars(length(max = 500))]
    pub annotations: Vec<VisualFeedbackAnnotation>,
    /// Current request's feedback PNG attachment; clear whenever the drawing changes.
    pub composite_attachment_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VisualFeedbackResult {
    pub source_version: String,
    pub width: u32,
    pub height: u32,
    #[schemars(length(max = 500))]
    pub annotations: Vec<VisualFeedbackAnnotation>,
    /// Resolve this ID through the package manifest attachments to view the composite PNG.
    pub composite_attachment_id: String,
}

pub(super) fn definition() -> WorkbenchDefinition {
    let mut definition = WorkbenchDefinition::new(
        WorkbenchSummary {
            kind: "visual_feedback",
            version: 1,
            name: "Visual feedback / 图像与画布反馈",
            purpose: "Draw on an immutable image or a native blank canvas and attach opinions.",
            returns: "A composite PNG attachment, structured freehand/arrow/rectangle/text annotations, and optional shared notes",
            interaction: "visual_feedback",
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
        input_schema: schemars::schema_for!(VisualFeedbackData),
        result_schema: schemars::schema_for!(VisualFeedbackResult),
        example: WorkbenchSpec {
            kind: "visual_feedback".into(),
            version: 1,
            data: WorkbenchData::VisualFeedback(VisualFeedbackData {
                title: "Sketch the next layout".into(),
                source_version: "draft-1".into(),
                width: 1280,
                height: 800,
                background_color: None,
                image_file_name: None,
            }),
        },
        instructions: "Use image_file_name:null for a native blank canvas. To review an existing image, include exactly one request attachment with that exact file_name and a correct PNG/JPEG/GIF/WebP extension; attachments[].path is read and copied durably when the request is created. The immutable image must have exactly the declared width and height (32–4096 integer pixels). Blank canvases use optional background_color:#RRGGBB, defaulting to white. Do not supply a live path or URL in workbench data. The human may draw freehand, arrows, rectangles, or text and attach optional opinions. At most 500 annotations and 20000 total points. Coordinates are integer canvas pixels. Shared notes remain in the feedback document. Submission returns a composite PNG referenced by composite_attachment_id in the package attachment manifest, plus structured annotations; at least one annotation or shared notes is required. No approval verdict or shortcut finish is supported.",
    })
}

pub(super) fn validate(data: &VisualFeedbackData) -> Result<(), ApplicationError> {
    super::validation::text("visual_feedback.title", &data.title, 200)?;
    super::validation::text("visual_feedback.source_version", &data.source_version, 128)?;
    if !(32..=4096).contains(&data.width) || !(32..=4096).contains(&data.height) {
        return Err(ApplicationError::invalid_argument(
            "visual_feedback canvas dimensions must be 32–4096 integer pixels",
        ));
    }
    if let Some(name) = &data.image_file_name {
        crate::workspace::validation::validate_file_name(name)?;
    }
    if data
        .background_color
        .as_ref()
        .is_some_and(|color| !color_valid(color))
    {
        return Err(ApplicationError::invalid_argument(
            "visual_feedback.background_color must be #RRGGBB",
        ));
    }
    Ok(())
}

fn color_valid(color: &str) -> bool {
    color.len() == 7
        && color.starts_with('#')
        && color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}

fn annotations_valid(
    data: &VisualFeedbackData,
    annotations: &[VisualFeedbackAnnotation],
    published: bool,
) -> bool {
    let mut ids = HashSet::new();
    annotations.len() <= 500
        && annotations
            .iter()
            .map(|annotation| annotation.points.len())
            .sum::<usize>()
            <= 20000
        && annotations.iter().all(|annotation| {
            let points = &annotation.points;
            super::validation::valid_item_id(&annotation.id)
                && ids.insert(&annotation.id)
                && color_valid(&annotation.color)
                && (1..=32).contains(&annotation.stroke_width)
                && !annotation.text.contains('\0')
                && !annotation.body.contains('\0')
                && annotation.text.chars().count() <= 2000
                && annotation.body.chars().count() <= 4000
                && points
                    .iter()
                    .all(|point| point.x <= data.width && point.y <= data.height)
                && match annotation.kind {
                    VisualFeedbackAnnotationKind::Freehand => {
                        (2..=2048).contains(&points.len()) && annotation.text.is_empty()
                    }
                    VisualFeedbackAnnotationKind::Arrow => {
                        points.len() == 2 && points[0] != points[1] && annotation.text.is_empty()
                    }
                    VisualFeedbackAnnotationKind::Rectangle => {
                        points.len() == 2
                            && points[0].x != points[1].x
                            && points[0].y != points[1].y
                            && annotation.text.is_empty()
                    }
                    VisualFeedbackAnnotationKind::Text => {
                        points.len() == 1 && (!published || !annotation.text.trim().is_empty())
                    }
                }
        })
}

pub(super) fn draft_valid(data: &VisualFeedbackData, state: Option<&WorkbenchState>) -> bool {
    match state {
        None => true,
        Some(WorkbenchState::VisualFeedback(state)) => {
            annotations_valid(data, &state.annotations, false)
                && state
                    .composite_attachment_id
                    .as_deref()
                    .is_none_or(super::validation::valid_item_id)
        }
        _ => false,
    }
}
pub(super) fn result(
    data: &VisualFeedbackData,
    state: Option<&WorkbenchState>,
) -> Option<WorkbenchResult> {
    let Some(WorkbenchState::VisualFeedback(state)) = state else {
        return None;
    };
    if !annotations_valid(data, &state.annotations, true) {
        return None;
    }
    let attachment_id = state.composite_attachment_id.as_ref()?;
    if !super::validation::valid_item_id(attachment_id) {
        return None;
    }
    Some(WorkbenchResult::VisualFeedback(VisualFeedbackResult {
        source_version: data.source_version.clone(),
        width: data.width,
        height: data.height,
        annotations: state.annotations.clone(),
        composite_attachment_id: attachment_id.clone(),
    }))
}
pub(super) fn has_input(data: &VisualFeedbackData, state: Option<&WorkbenchState>) -> bool {
    matches!(state, Some(WorkbenchState::VisualFeedback(state)) if !state.annotations.is_empty() && annotations_valid(data, &state.annotations, false) && state.composite_attachment_id.as_deref().is_none_or(super::validation::valid_item_id))
}
pub(super) fn complete(data: &VisualFeedbackData, result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::VisualFeedback(result)) if result.source_version == data.source_version && result.width == data.width && result.height == data.height && annotations_valid(data, &result.annotations, true) && super::validation::valid_item_id(&result.composite_attachment_id))
}
pub(super) fn result_has_input(result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::VisualFeedback(result)) if !result.annotations.is_empty())
}
pub(super) fn legacy_actions(_: &VisualFeedbackData) -> Vec<ActionInput> {
    vec![]
}

/// Validate the immutable material after request attachment bytes have been loaded.
pub fn validate_visual_feedback_material(
    spec: Option<&WorkbenchSpec>,
    attachments: &[crate::NewRequestAttachment],
) -> Result<(), ApplicationError> {
    let Some(WorkbenchSpec {
        data: WorkbenchData::VisualFeedback(data),
        ..
    }) = spec
    else {
        return Ok(());
    };
    let Some(name) = &data.image_file_name else {
        return Ok(());
    };
    let mut matches = attachments
        .iter()
        .filter(|attachment| &attachment.file_name == name);
    let attachment = matches.next().ok_or_else(|| ApplicationError::invalid_argument("visual_feedback.image_file_name must match exactly one immutable request image attachment"))?;
    if matches.next().is_some() || !attachment.media_type.starts_with("image/") {
        return Err(ApplicationError::invalid_argument(
            "visual_feedback.image_file_name must match exactly one immutable request image attachment",
        ));
    }
    if visual_feedback_image_dimensions(&attachment.contents, &attachment.media_type)?
        != (data.width, data.height)
    {
        return Err(ApplicationError::invalid_argument(
            "visual_feedback request image dimensions must match the declared canvas width and height",
        ));
    }
    Ok(())
}

/// Fully decode bounded image bytes and report their EXIF-oriented display dimensions.
pub fn visual_feedback_image_dimensions(
    contents: &[u8],
    media_type: &str,
) -> Result<(u32, u32), ApplicationError> {
    use image::{
        DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, metadata::Orientation,
    };
    let format = match media_type {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/gif" => ImageFormat::Gif,
        "image/webp" => ImageFormat::WebP,
        _ => {
            return Err(ApplicationError::invalid_argument(
                "visual_feedback requires a PNG, JPEG, GIF or WebP image",
            ));
        }
    };
    if contents.is_empty() || contents.len() > crate::MAX_ATTACHMENT_BYTES {
        return Err(ApplicationError::invalid_argument(
            "visual_feedback image exceeds its attachment byte limit",
        ));
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(128 * 1024 * 1024);
    let mut reader = ImageReader::with_format(std::io::Cursor::new(contents), format);
    reader.limits(limits.clone());
    let invalid_image = || {
        ApplicationError::invalid_argument(
            "visual_feedback image must decode completely within 4096x4096 pixels and its memory limit",
        )
    };
    let mut decoder = reader.into_decoder().map_err(|_| invalid_image())?;
    let orientation = decoder.orientation().map_err(|_| invalid_image())?;
    limits
        .reserve(decoder.total_bytes())
        .map_err(|_| invalid_image())?;
    decoder.set_limits(limits).map_err(|_| invalid_image())?;
    let image = DynamicImage::from_decoder(decoder).map_err(|_| invalid_image())?;
    if matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    ) {
        Ok((image.height(), image.width()))
    } else {
        Ok((image.width(), image.height()))
    }
}
