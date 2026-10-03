use super::*;
use std::fmt::Write as _;

const MAX_BYTES: usize = 32 * 1024;
const TRUNCATED: &str = "\n[Operation details truncated]";

pub(super) fn hydrate(permission: &mut SessionInteraction, stream: &StreamState) {
    let SessionInteractionKind::Permission { tool_call_id, .. } = &permission.kind else {
        return;
    };
    // Only the current turn's exact tool is an authoritative fallback. Never
    // search persisted history or choose the most recent command by position.
    let tool = tool_call_id
        .as_ref()
        .and_then(|id| stream.tools.get(id))
        .filter(|row| {
            row.session_id == permission.session_id
                && stream.turn_id.is_some()
                && row.turn_id == stream.turn_id
        })
        .and_then(|row| match row.content.as_ref() {
            Some(SessionActivityContent::ToolCall { tool })
                if Some(&tool.id) == tool_call_id.as_ref() =>
            {
                Some(tool)
            }
            _ => None,
        });
    if let Some(tool) = tool {
        if permission.title.trim().is_empty() {
            permission.title = tool.title.clone();
        }
        if permission.details.is_none() {
            permission.details = describe(tool);
        }
    }
    if permission.title.trim().is_empty() {
        permission.title = "Agent tool operation".into();
    }
}

// Display only. Do not expose previous tool output as requested input, open
// referenced resources, or execute anything while preparing the consent card.
fn describe(tool: &SessionToolCall) -> Option<String> {
    let mut out = Details::default();
    if let Some(input) = &tool.raw_input {
        let _ = writeln!(out, "Input:\n{input}");
    }
    for location in &tool.locations {
        let _ = write!(out, "Location: {}", location.path);
        if let Some(line) = location.line {
            let _ = write!(out, ":{line}");
        }
        let _ = writeln!(out);
    }
    for block in &tool.content {
        match block {
            SessionContentBlock::Diff {
                path,
                old_text,
                new_text,
            } => {
                let _ = writeln!(out, "File change: {path}");
                if let Some(old) = old_text {
                    let _ = writeln!(out, "Before:\n{old}");
                }
                let _ = writeln!(out, "After:\n{new_text}");
            }
            SessionContentBlock::Text { text } => {
                let _ = writeln!(out, "Content:\n{text}");
            }
            SessionContentBlock::Resource { uri, .. } => {
                let _ = writeln!(out, "Resource: {uri}");
            }
            SessionContentBlock::Terminal { terminal_id } => {
                let _ = writeln!(out, "Terminal reference: {terminal_id}");
            }
            _ => {}
        }
    }
    if out.truncated || tool.truncated {
        out.text.push_str(TRUNCATED);
    }
    (!out.text.trim().is_empty()).then_some(out.text)
}

#[derive(Default)]
struct Details {
    text: String,
    truncated: bool,
}

impl std::fmt::Write for Details {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        if self.truncated {
            return Err(std::fmt::Error);
        }
        let mut end = value
            .len()
            .min(MAX_BYTES - TRUNCATED.len() - self.text.len());
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        self.text.push_str(&value[..end]);
        self.truncated = end < value.len();
        if self.truncated {
            Err(std::fmt::Error)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "permission_context_tests.rs"]
mod tests;
