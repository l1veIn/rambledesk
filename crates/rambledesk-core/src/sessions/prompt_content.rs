//! Typed prompt inputs. Unlike display previews these are rejected, never
//! truncated, before a turn starts or anything is sent to the Agent.
use super::*;
use base64::Engine;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const MAX_PROMPT_CONTENT_BLOCKS: usize = 16;
pub const MAX_PROMPT_CONTENT_BYTES: usize = 4 * 1024 * 1024;
/// Encoded base64 bytes, approximately 1.5 MiB of decoded image data.
pub const MAX_PROMPT_IMAGE_BASE64_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_PROMPT_TEXT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AgentPromptCapabilities {
    pub image: bool,
    pub audio: bool,
    pub embedded_context: bool,
    /// True for ACP's mandatory resource-link baseline; false for a driver
    /// which has not declared support for typed resource inputs.
    pub resource_links: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionPromptContent {
    Text {
        text: String,
    },
    Image {
        mime_type: String,
        data: String,
    },
    ResourceLink {
        uri: String,
        name: String,
        mime_type: Option<String>,
    },
    Resource {
        uri: String,
        mime_type: Option<String>,
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SendManagedPromptContentInput {
    pub session_id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub content: Vec<SessionPromptContent>,
}

impl SendManagedPromptContentInput {
    pub(super) fn into_blocks(self) -> Result<Vec<SessionPromptContent>, SessionError> {
        let mut blocks = self.content;
        if !self.text.is_empty() {
            blocks.insert(0, SessionPromptContent::Text { text: self.text });
        }
        validate_prompt_content(&blocks).map_err(|_| SessionError::InvalidInput)?;
        Ok(blocks)
    }
}

pub fn validate_prompt_content(blocks: &[SessionPromptContent]) -> Result<(), AgentDriverError> {
    let invalid = || {
        AgentDriverError::new("Prompt content exceeds supported bounds or has an invalid format")
    };
    if blocks.is_empty() || blocks.len() > MAX_PROMPT_CONTENT_BLOCKS {
        return Err(invalid());
    }
    let mut total = 0_usize;
    let mut text_bytes = 0_usize;
    let mut meaningful = false;
    for block in blocks {
        let size = match block {
            SessionPromptContent::Text { text } => {
                text_bytes += text.len();
                meaningful |= !text.trim().is_empty();
                text.len()
            }
            SessionPromptContent::Image { mime_type, data } => {
                if data.len() > MAX_PROMPT_IMAGE_BASE64_BYTES {
                    return Err(invalid());
                }
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .map_err(|_| invalid())?;
                let valid_header = match mime_type.as_str() {
                    "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
                    "image/jpeg" => bytes.starts_with(b"\xff\xd8\xff"),
                    "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
                    "image/webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
                    _ => false,
                };
                if !valid_header {
                    return Err(invalid());
                }
                meaningful = true;
                data.len() + mime_type.len()
            }
            SessionPromptContent::ResourceLink {
                uri,
                name,
                mime_type,
            } => {
                if !valid_uri(uri)
                    || name.trim().is_empty()
                    || name.len() > 1024
                    || name.contains('\0')
                    || !valid_mime(mime_type)
                {
                    return Err(invalid());
                }
                meaningful = true;
                uri.len() + name.len() + mime_type.as_ref().map_or(0, String::len)
            }
            SessionPromptContent::Resource {
                uri,
                mime_type,
                text,
            } => {
                if !valid_uri(uri) || !valid_mime(mime_type) {
                    return Err(invalid());
                }
                text_bytes += text.len();
                meaningful = true;
                uri.len() + text.len() + mime_type.as_ref().map_or(0, String::len)
            }
        };
        total = total.checked_add(size).ok_or_else(invalid)?;
        if total > MAX_PROMPT_CONTENT_BYTES || text_bytes > MAX_PROMPT_TEXT_BYTES {
            return Err(invalid());
        }
    }
    if !meaningful {
        return Err(invalid());
    }
    Ok(())
}

fn valid_uri(uri: &str) -> bool {
    if uri.len() > 8192
        || uri
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace() || ch == '\\')
    {
        return false;
    }
    uri.split_once("://").is_some_and(|(scheme, rest)| {
        matches!(scheme, "file" | "http" | "https") && !rest.is_empty()
    })
}
fn valid_mime(mime: &Option<String>) -> bool {
    mime.as_deref().is_none_or(|mime| {
        mime.len() <= 256 && mime.contains('/') && mime.bytes().all(|byte| byte.is_ascii_graphic())
    })
}

pub fn prompt_content_supported(
    blocks: &[SessionPromptContent],
    capabilities: &AgentPromptCapabilities,
) -> bool {
    blocks.iter().all(|block| match block {
        SessionPromptContent::Text { .. } => true,
        SessionPromptContent::Image { .. } => capabilities.image,
        SessionPromptContent::ResourceLink { .. } => capabilities.resource_links,
        SessionPromptContent::Resource { .. } => capabilities.embedded_context,
    })
}

pub(super) fn prompt_display(blocks: &[SessionPromptContent]) -> SessionActivityContent {
    let mut out = vec![];
    let mut truncated = false;
    // Metadata and text are already bounded. Reserve them before deciding which
    // binary previews fit, so an earlier image cannot hide subsequent text.
    let mut remaining = MAX_ACTIVITY_CONTENT_BYTES;
    for block in blocks {
        let display = match block {
            SessionPromptContent::Text { text } => SessionContentBlock::Text { text: text.clone() },
            SessionPromptContent::Image { mime_type, .. } => SessionContentBlock::Image {
                mime_type: mime_type.clone(),
                data: None,
                uri: None,
            },
            SessionPromptContent::ResourceLink {
                uri,
                name,
                mime_type,
            } => SessionContentBlock::Resource {
                uri: uri.clone(),
                name: Some(name.clone()),
                mime_type: mime_type.clone(),
                text: None,
            },
            SessionPromptContent::Resource {
                uri,
                mime_type,
                text,
            } => SessionContentBlock::Resource {
                uri: uri.clone(),
                name: None,
                mime_type: mime_type.clone(),
                text: Some(text.clone()),
            },
        };
        remaining = remaining.saturating_sub(display.byte_len());
        out.push(display);
    }
    for (source, display) in blocks.iter().zip(&mut out) {
        if let (
            SessionPromptContent::Image { data, .. },
            SessionContentBlock::Image { data: preview, .. },
        ) = (source, display)
        {
            if data.len() <= MAX_INLINE_MEDIA_BASE64_BYTES && data.len() <= remaining {
                remaining -= data.len();
                *preview = Some(data.clone());
            } else {
                truncated = true;
            }
        }
    }
    SessionActivityContent::Message {
        blocks: out,
        truncated,
    }
}

#[cfg(test)]
#[path = "prompt_content_tests.rs"]
mod tests;
