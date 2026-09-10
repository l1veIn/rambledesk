//! Durable display content, independent of ACP wire types.
//!
//! Field-preserving tool updates and first-occurrence ordering are informed by
//! Codeg 3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1, session_state.rs
//! (ToolCallState/upsert_tool_call). This implementation uses our persistent
//! activity/turn model; it does not copy Codeg's transient transcript ownership.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const MAX_ACTIVITY_CONTENT_BYTES: usize = 512 * 1024;
pub const MAX_ACTIVITY_TEXT_BYTES: usize = 256 * 1024;
pub const MAX_ACTIVITY_CONTENT_BLOCKS: usize = 64;
pub const MAX_TOOL_RAW_BYTES: usize = 64 * 1024;
pub const MAX_INLINE_MEDIA_BASE64_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum SessionActivityContent {
    Message {
        blocks: Vec<SessionContentBlock>,
        #[serde(default)]
        truncated: bool,
    },
    ToolCall {
        tool: SessionToolCall,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum SessionContentBlock {
    Text {
        text: String,
    },
    Image {
        mime_type: String,
        data: Option<String>,
        uri: Option<String>,
    },
    Audio {
        mime_type: String,
        data: Option<String>,
    },
    Resource {
        uri: String,
        name: Option<String>,
        mime_type: Option<String>,
        text: Option<String>,
    },
    Diff {
        path: String,
        old_text: Option<String>,
        new_text: String,
    },
    Terminal {
        terminal_id: String,
    },
    Unsupported {
        label: String,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum SessionToolKind {
    Read,
    Edit,
    Delete,
    Move,
    Search,
    Execute,
    Think,
    Fetch,
    #[default]
    Other,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum SessionToolStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl SessionToolStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionToolLocation {
    pub path: String,
    pub line: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionToolCall {
    pub id: String,
    pub name: Option<String>,
    pub title: String,
    pub kind: SessionToolKind,
    pub status: SessionToolStatus,
    /// Bounded serialized JSON for display, not executable input. If truncated,
    /// the string is a preview and need not parse as a complete JSON value.
    pub raw_input: Option<String>,
    pub raw_output: Option<String>,
    pub content: Vec<SessionContentBlock>,
    pub locations: Vec<SessionToolLocation>,
    #[serde(default)]
    pub truncated: bool,
}

/// Absent fields preserve the preceding value. Present empty vectors replace
/// it with an empty collection. Each raw input/output is a whole-value update.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionToolCallPatch {
    pub name: Option<String>,
    pub title: Option<String>,
    pub kind: Option<SessionToolKind>,
    pub status: Option<SessionToolStatus>,
    pub raw_input: Option<String>,
    pub raw_output: Option<String>,
    pub content: Option<Vec<SessionContentBlock>>,
    pub locations: Option<Vec<SessionToolLocation>>,
    pub truncated: bool,
}

impl SessionToolCall {
    pub fn new(id: String) -> Self {
        Self {
            id,
            name: None,
            title: "Agent tool operation".into(),
            kind: SessionToolKind::Other,
            status: SessionToolStatus::Pending,
            raw_input: None,
            raw_output: None,
            content: vec![],
            locations: vec![],
            truncated: false,
        }
    }

    pub fn apply_patch(&mut self, patch: SessionToolCallPatch) {
        if let Some(value) = patch.name {
            self.name = Some(value);
        }
        if let Some(value) = patch.title {
            self.title = value;
        }
        if let Some(value) = patch.kind {
            self.kind = value;
        }
        if let Some(value) = patch.status {
            self.status = value;
        }
        if let Some(value) = patch.raw_input {
            self.raw_input = Some(value);
        }
        if let Some(value) = patch.raw_output {
            self.raw_output = Some(value);
        }
        if let Some(value) = patch.content {
            self.content = value;
        }
        if let Some(value) = patch.locations {
            self.locations = value;
        }
        self.truncated |= patch.truncated;
    }
}

impl SessionContentBlock {
    pub fn byte_len(&self) -> usize {
        let optional = |value: &Option<String>| value.as_ref().map_or(0, String::len);
        match self {
            Self::Text { text } => text.len(),
            Self::Image {
                mime_type,
                data,
                uri,
            } => mime_type.len() + optional(data) + optional(uri),
            Self::Audio { mime_type, data } => mime_type.len() + optional(data),
            Self::Resource {
                uri,
                name,
                mime_type,
                text,
            } => uri.len() + optional(name) + optional(mime_type) + optional(text),
            Self::Diff {
                path,
                old_text,
                new_text,
            } => path.len() + optional(old_text) + new_text.len(),
            Self::Terminal { terminal_id } => terminal_id.len(),
            Self::Unsupported { label } => label.len(),
        }
    }

    pub fn summary(&self) -> String {
        match self {
            Self::Text { text } => text.clone(),
            Self::Image { mime_type, .. } => format!("[Image: {mime_type}]"),
            Self::Audio { mime_type, .. } => format!("[Audio: {mime_type}]"),
            Self::Resource { uri, text, .. } => format!(
                "[Resource: {uri}]{}",
                text.as_ref()
                    .map(|text| format!("\n{text}"))
                    .unwrap_or_default()
            ),
            Self::Diff { path, .. } => format!("[File change: {path}]"),
            Self::Terminal { terminal_id } => format!("[Terminal: {terminal_id}]"),
            Self::Unsupported { label } => format!("[{label}]"),
        }
    }
}

impl SessionActivityContent {
    pub fn summary(&self) -> String {
        match self {
            Self::Message { blocks, .. } => blocks
                .iter()
                .map(SessionContentBlock::summary)
                .collect::<Vec<_>>()
                .join("\n"),
            Self::ToolCall { tool } => format!("{} · {}", tool.title, tool.status.as_str()),
        }
    }

    pub fn valid_size(&self) -> bool {
        let (blocks, bytes) = match self {
            Self::Message { blocks, .. } => (blocks, 0),
            Self::ToolCall { tool } => {
                if tool.locations.len() > 64
                    || tool
                        .raw_input
                        .as_ref()
                        .is_some_and(|raw| raw.len() > MAX_TOOL_RAW_BYTES)
                    || tool
                        .raw_output
                        .as_ref()
                        .is_some_and(|raw| raw.len() > MAX_TOOL_RAW_BYTES)
                {
                    return false;
                }
                let bytes = tool.id.len()
                    + tool.title.len()
                    + tool.name.as_ref().map_or(0, String::len)
                    + tool.raw_input.as_ref().map_or(0, String::len)
                    + tool.raw_output.as_ref().map_or(0, String::len)
                    + tool
                        .locations
                        .iter()
                        .map(|location| location.path.len())
                        .sum::<usize>();
                (&tool.content, bytes)
            }
        };
        blocks.len() <= MAX_ACTIVITY_CONTENT_BLOCKS
            && blocks.iter().all(|block| match block {
                SessionContentBlock::Image { data, .. }
                | SessionContentBlock::Audio { data, .. } => data
                    .as_ref()
                    .is_none_or(|data| data.len() <= MAX_INLINE_MEDIA_BASE64_BYTES),
                _ => true,
            })
            && bytes
                + blocks
                    .iter()
                    .map(SessionContentBlock::byte_len)
                    .sum::<usize>()
                <= MAX_ACTIVITY_CONTENT_BYTES
    }
}
