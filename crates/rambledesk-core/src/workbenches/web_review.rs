use super::*;
use std::collections::HashSet;

/// Initial page and viewport. The website remains live and may navigate or change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WebReviewData {
    #[schemars(length(min = 1, max = 200))]
    pub title: String,
    #[schemars(length(min = 1, max = 8192))]
    pub url: String,
    #[schemars(length(min = 1, max = 128))]
    pub source_version: String,
    pub viewport: WebReviewViewport,
}

/// Integer CSS pixels; overlays must not change this viewport implicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WebReviewViewport {
    #[schemars(range(min = 240, max = 7680))]
    pub width: u32,
    #[schemars(range(min = 200, max = 4320))]
    pub height: u32,
}

/// Rounded CSS pixels relative to the document at selection time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WebReviewRect {
    #[schemars(range(min = -1000000, max = 1000000))]
    pub x: i32,
    #[schemars(range(min = -1000000, max = 1000000))]
    pub y: i32,
    #[schemars(range(min = 1, max = 1000000))]
    pub width: u32,
    #[schemars(range(min = 1, max = 1000000))]
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WebReviewElement {
    /// Untrusted CSS selector hint, not proof of a DOM element or site identity.
    #[schemars(length(min = 1, max = 2000))]
    pub selector: String,
    #[schemars(regex(pattern = "^[a-z][a-z0-9-]{0,63}$"))]
    pub tag_name: String,
    #[schemars(length(max = 2000))]
    pub text: String,
    pub rect: WebReviewRect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WebReviewAnnotation {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,63}$"))]
    pub id: String,
    #[schemars(length(min = 1, max = 8192))]
    pub page_url: String,
    pub viewport: WebReviewViewport,
    pub element: WebReviewElement,
    #[schemars(length(min = 1, max = 4000))]
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,63}$"))]
    pub screenshot_attachment_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WebReviewResult {
    pub source_version: String,
    #[schemars(length(max = 500))]
    pub annotations: Vec<WebReviewAnnotation>,
}

pub(super) fn web_review_url_valid(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://"))
        || value.contains('\\')
        || value.chars().count() > 8192
        || value.chars().any(|character| {
            character.is_control() || character.is_whitespace() || character == '\u{feff}'
        })
    {
        return false;
    }
    url::Url::parse(value).is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
    })
}

pub(super) fn web_review_viewport_valid(viewport: &WebReviewViewport) -> bool {
    (240..=7680).contains(&viewport.width) && (200..=4320).contains(&viewport.height)
}

pub(super) fn web_review_annotations_valid(annotations: &[WebReviewAnnotation]) -> bool {
    let mut ids = HashSet::new();
    annotations.len() <= 500
        && annotations.iter().all(|annotation| {
            let element = &annotation.element;
            let rect = &element.rect;
            let tag = element.tag_name.as_bytes();
            super::validation::valid_item_id(&annotation.id)
                && ids.insert(&annotation.id)
                && web_review_url_valid(&annotation.page_url)
                && web_review_viewport_valid(&annotation.viewport)
                && !annotation.body.trim().is_empty()
                && annotation.body.chars().count() <= 4000
                && !annotation.body.contains('\0')
                && !element.selector.trim().is_empty()
                && element.selector.chars().count() <= 2000
                && !element.selector.contains('\0')
                && !tag.is_empty()
                && tag.len() <= 64
                && tag[0].is_ascii_lowercase()
                && tag
                    .iter()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
                && element.text.chars().count() <= 2000
                && !element.text.contains('\0')
                && (-1000000..=1000000).contains(&rect.x)
                && (-1000000..=1000000).contains(&rect.y)
                && (1..=1000000).contains(&rect.width)
                && (1..=1000000).contains(&rect.height)
                && annotation
                    .screenshot_attachment_id
                    .as_deref()
                    .is_none_or(super::validation::valid_item_id)
        })
}
