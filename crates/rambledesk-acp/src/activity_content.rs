//! ACP display normalization. Field-preserving upserts and first-occurrence
//! anchors follow Codeg's session_state.rs at 3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1.
//! Standard ACP raw values replace whole fields; no vendor chunk heuristic.
use agent_client_protocol::schema::v1 as acp;
use rambledesk_core::*;

use crate::AcpError;

pub(crate) fn convert(update: acp::SessionUpdate) -> Result<Option<AgentSessionEvent>, AcpError> {
    let event = match update {
        acp::SessionUpdate::UsageUpdate(usage) => {
            AgentSessionEvent::ContextUsage(SessionContextUsage {
                used: usage.used,
                size: usage.size,
            })
        }
        acp::SessionUpdate::AgentMessageChunk(chunk) => {
            message(SessionActivityKind::AgentMessage, &chunk.content)
        }
        acp::SessionUpdate::AgentThoughtChunk(chunk) => {
            message(SessionActivityKind::AgentThought, &chunk.content)
        }
        acp::SessionUpdate::ToolCall(tool) => {
            let mut fields = acp::ToolCallUpdateFields::new();
            fields.title = Some(tool.title);
            fields.kind = Some(tool.kind);
            fields.status = Some(tool.status);
            fields.content = Some(tool.content);
            fields.locations = Some(tool.locations);
            fields.raw_input = tool.raw_input;
            fields.raw_output = tool.raw_output;
            tool_event(tool.tool_call_id.to_string(), &fields, tool.meta.as_ref())?
        }
        acp::SessionUpdate::ToolCallUpdate(tool) => tool_event(
            tool.tool_call_id.to_string(),
            &tool.fields,
            tool.meta.as_ref(),
        )?,
        // User chunks echo input already persisted locally. Capability and
        // configuration notifications are not transcript entries.
        _ => return Ok(None),
    };
    Ok(Some(event))
}

fn message(kind: SessionActivityKind, content: &acp::ContentBlock) -> AgentSessionEvent {
    let mut budget = Budget::new(MAX_ACTIVITY_TEXT_BYTES);
    let block = budget.content(content);
    AgentSessionEvent::MessageChunk {
        kind,
        block,
        truncated: budget.truncated,
    }
}

fn tool_event(
    id: String,
    fields: &acp::ToolCallUpdateFields,
    meta: Option<&acp::Meta>,
) -> Result<AgentSessionEvent, AcpError> {
    // IDs are routing identity: reject pathological IDs rather than truncate
    // them and accidentally merge distinct tools.
    if id.is_empty() || id.len() > 1024 || id.contains('\0') {
        return Err(AcpError::Protocol("invalid tool identity"));
    }
    let mut metadata = Budget::new(70 * 1024);
    let name = meta
        .and_then(|meta| {
            ["toolName", "tool_name", "codebuddy.ai/toolName"]
                .iter()
                .find_map(|key| meta.get(*key).and_then(serde_json::Value::as_str))
        })
        .map(|name| metadata.text(name, 1024));
    let title = fields
        .title
        .as_ref()
        .map(|title| metadata.text(title, 4096));
    let locations = fields.locations.as_ref().map(|locations| {
        metadata.truncated |= locations.len() > 64;
        locations
            .iter()
            .take(64)
            .map(|location| SessionToolLocation {
                path: metadata.text(&location.path.to_string_lossy(), 1024),
                line: location.line,
            })
            .collect()
    });
    let mut budget = Budget::new(MAX_ACTIVITY_TEXT_BYTES);
    let content = fields.content.as_ref().map(|content| {
        budget.truncated |= content.len() > MAX_ACTIVITY_CONTENT_BLOCKS;
        content
            .iter()
            .take(MAX_ACTIVITY_CONTENT_BLOCKS)
            .map(|content| match content {
                acp::ToolCallContent::Content(content) => budget.content(&content.content),
                acp::ToolCallContent::Diff(diff) => SessionContentBlock::Diff {
                    path: budget.text(&diff.path.to_string_lossy(), 4096),
                    old_text: diff
                        .old_text
                        .as_ref()
                        .map(|text| budget.text(text, MAX_ACTIVITY_TEXT_BYTES / 2)),
                    new_text: budget.text(&diff.new_text, MAX_ACTIVITY_TEXT_BYTES / 2),
                },
                acp::ToolCallContent::Terminal(terminal) => SessionContentBlock::Terminal {
                    terminal_id: budget.text(&terminal.terminal_id.to_string(), 1024),
                },
                _ => SessionContentBlock::Unsupported {
                    label: "Unsupported tool content".into(),
                },
            })
            .collect()
    });
    let mut truncated = budget.truncated || metadata.truncated;
    let raw_input = fields
        .raw_input
        .as_ref()
        .map(|value| raw_preview(value, &mut truncated));
    let raw_output = fields
        .raw_output
        .as_ref()
        .map(|value| raw_preview(value, &mut truncated));
    Ok(AgentSessionEvent::ToolCall {
        tool_call_id: id,
        patch: SessionToolCallPatch {
            name,
            title,
            kind: fields.kind.map(kind),
            status: fields.status.map(status),
            raw_input,
            raw_output,
            content,
            locations,
            truncated,
        },
    })
}

fn kind(kind: acp::ToolKind) -> SessionToolKind {
    match kind {
        acp::ToolKind::Read => SessionToolKind::Read,
        acp::ToolKind::Edit => SessionToolKind::Edit,
        acp::ToolKind::Delete => SessionToolKind::Delete,
        acp::ToolKind::Move => SessionToolKind::Move,
        acp::ToolKind::Search => SessionToolKind::Search,
        acp::ToolKind::Execute => SessionToolKind::Execute,
        acp::ToolKind::Think => SessionToolKind::Think,
        acp::ToolKind::Fetch => SessionToolKind::Fetch,
        _ => SessionToolKind::Other,
    }
}
fn status(status: acp::ToolCallStatus) -> SessionToolStatus {
    match status {
        acp::ToolCallStatus::Pending => SessionToolStatus::Pending,
        acp::ToolCallStatus::InProgress => SessionToolStatus::InProgress,
        acp::ToolCallStatus::Completed => SessionToolStatus::Completed,
        acp::ToolCallStatus::Failed => SessionToolStatus::Failed,
        _ => SessionToolStatus::Pending,
    }
}

struct Budget {
    remaining: usize,
    truncated: bool,
}
impl Budget {
    fn new(remaining: usize) -> Self {
        Self {
            remaining,
            truncated: false,
        }
    }
    fn text(&mut self, text: &str, limit: usize) -> String {
        let mut end = text.len().min(limit).min(self.remaining);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        self.truncated |= end < text.len();
        self.remaining -= end;
        text[..end].into()
    }
    fn media(&mut self, data: &str) -> Option<String> {
        // Never persist partial base64. The metadata survives an omitted blob.
        if data.len() > MAX_INLINE_MEDIA_BASE64_BYTES || data.len() > self.remaining {
            self.truncated = true;
            None
        } else {
            self.remaining -= data.len();
            Some(data.into())
        }
    }
    fn content(&mut self, content: &acp::ContentBlock) -> SessionContentBlock {
        match content {
            acp::ContentBlock::Text(text) => SessionContentBlock::Text {
                text: self.text(&text.text, MAX_ACTIVITY_TEXT_BYTES),
            },
            acp::ContentBlock::Image(image) => SessionContentBlock::Image {
                mime_type: self.text(&image.mime_type, 256),
                uri: image.uri.as_ref().map(|uri| self.text(uri, 4096)),
                data: self.media(&image.data),
            },
            acp::ContentBlock::Audio(audio) => SessionContentBlock::Audio {
                mime_type: self.text(&audio.mime_type, 256),
                data: self.media(&audio.data),
            },
            acp::ContentBlock::ResourceLink(link) => SessionContentBlock::Resource {
                uri: self.text(&link.uri, 4096),
                name: Some(self.text(&link.name, 1024)),
                mime_type: link.mime_type.as_ref().map(|mime| self.text(mime, 256)),
                text: None,
            },
            acp::ContentBlock::Resource(resource) => match &resource.resource {
                acp::EmbeddedResourceResource::TextResourceContents(text) => {
                    SessionContentBlock::Resource {
                        uri: self.text(&text.uri, 4096),
                        name: None,
                        mime_type: text.mime_type.as_ref().map(|mime| self.text(mime, 256)),
                        text: Some(self.text(&text.text, MAX_ACTIVITY_TEXT_BYTES)),
                    }
                }
                acp::EmbeddedResourceResource::BlobResourceContents(blob) => {
                    self.truncated |= !blob.blob.is_empty();
                    SessionContentBlock::Resource {
                        uri: self.text(&blob.uri, 4096),
                        name: None,
                        mime_type: blob.mime_type.as_ref().map(|mime| self.text(mime, 256)),
                        text: None,
                    }
                }
                _ => SessionContentBlock::Unsupported {
                    label: "Unsupported resource".into(),
                },
            },
            _ => SessionContentBlock::Unsupported {
                label: "Unsupported agent content".into(),
            },
        }
    }
}

fn raw_preview(value: &serde_json::Value, truncated: &mut bool) -> String {
    let mut writer = JsonPreview {
        bytes: vec![],
        truncated: false,
    };
    let _ = serde_json::to_writer(&mut writer, value);
    *truncated |= writer.truncated;
    let end = match std::str::from_utf8(&writer.bytes) {
        Ok(_) => writer.bytes.len(),
        Err(error) => error.valid_up_to(),
    };
    writer.bytes.truncate(end);
    String::from_utf8(writer.bytes).expect("valid UTF-8 prefix")
}
struct JsonPreview {
    bytes: Vec<u8>,
    truncated: bool,
}
impl std::io::Write for JsonPreview {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let len = bytes.len().min(MAX_TOOL_RAW_BYTES - self.bytes.len());
        self.bytes.extend_from_slice(&bytes[..len]);
        if len < bytes.len() {
            self.truncated = true;
            Err(std::io::Error::other("tool display limit reached"))
        } else {
            Ok(len)
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "activity_content_tests.rs"]
mod tests;
