use agent_client_protocol::schema::v1::{
    ContentBlock, EmbeddedResourceResource, ToolCallContent, ToolCallUpdateFields,
};
use std::fmt::Write as _;

const MAX_BYTES: usize = 32 * 1024;
const TRUNCATED: &str = "\n[Operation details truncated at 32 KiB]";

/// These strings are untrusted display text. No resources are opened and no
/// terminal/filesystem client capability is implied by describing a reference.
pub(crate) fn describe(fields: &ToolCallUpdateFields) -> Option<String> {
    let mut out = LimitedText::default();
    for location in fields.locations.iter().flatten() {
        let _ = write!(out, "Location: {}", location.path.display());
        if let Some(line) = location.line {
            let _ = write!(out, ":{line}");
        }
        out.push("\n");
        if out.truncated {
            return out.finish();
        }
    }
    if let Some(input) = fields.raw_input.as_ref().filter(|input| !empty_json(input)) {
        out.push("Input:\n");
        // Abort serialization as soon as the bounded sink fills; never build an
        // unbounded intermediate copy of arbitrary backend JSON or binary data.
        let _ = serde_json::to_writer_pretty(&mut out, input);
        out.push("\n");
    }
    for content in fields.content.iter().flatten() {
        if out.truncated {
            break;
        }
        match content {
            ToolCallContent::Content(content) => describe_content(&mut out, &content.content),
            ToolCallContent::Diff(diff) => {
                let _ = writeln!(out, "File change: {}", diff.path.display());
                if let Some(old) = &diff.old_text {
                    out.push("Before:\n");
                    out.push(old);
                    out.push("\n");
                }
                out.push("After:\n");
                out.push(&diff.new_text);
                out.push("\n");
            }
            ToolCallContent::Terminal(terminal) => {
                let _ = writeln!(out, "Terminal reference: {}", terminal.terminal_id);
            }
            _ => out.push("[Unsupported tool content]\n"),
        }
    }
    out.finish()
}

fn describe_content(out: &mut LimitedText, content: &ContentBlock) {
    match content {
        ContentBlock::Text(text) if !text.text.trim().is_empty() => {
            out.push("Content:\n");
            out.push(&text.text);
            out.push("\n");
        }
        ContentBlock::Text(_) => {}
        ContentBlock::ResourceLink(link) => {
            let _ = writeln!(out, "Resource: {}", link.uri);
        }
        ContentBlock::Resource(resource) => match &resource.resource {
            EmbeddedResourceResource::TextResourceContents(text) => {
                let _ = writeln!(out, "Resource: {}", text.uri);
                out.push(&text.text);
                out.push("\n");
            }
            EmbeddedResourceResource::BlobResourceContents(blob) => {
                let _ = writeln!(out, "Binary resource: {}", blob.uri);
            }
            _ => out.push("[Unsupported resource content]\n"),
        },
        ContentBlock::Image(_) => out.push("[Image content]\n"),
        ContentBlock::Audio(_) => out.push("[Audio content]\n"),
        _ => out.push("[Unsupported content]\n"),
    }
}

fn empty_json(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => true,
        serde_json::Value::String(text) => text.trim().is_empty(),
        serde_json::Value::Array(items) => items.is_empty(),
        serde_json::Value::Object(fields) => fields.is_empty(),
        _ => false,
    }
}

#[derive(Default)]
struct LimitedText {
    text: String,
    truncated: bool,
}

impl LimitedText {
    fn push(&mut self, value: &str) {
        if self.truncated {
            return;
        }
        let remaining = MAX_BYTES - TRUNCATED.len() - self.text.len();
        let mut end = remaining.min(value.len());
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        self.text.push_str(&value[..end]);
        self.truncated = end < value.len();
    }

    fn finish(mut self) -> Option<String> {
        if self.truncated {
            self.text.push_str(TRUNCATED);
        }
        (!self.text.trim().is_empty()).then_some(self.text)
    }
}

impl std::fmt::Write for LimitedText {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.push(value);
        if self.truncated {
            Err(std::fmt::Error)
        } else {
            Ok(())
        }
    }
}

impl std::io::Write for LimitedText {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let text = std::str::from_utf8(bytes).map_err(std::io::Error::other)?;
        self.push(text);
        if self.truncated {
            Err(std::io::Error::other("permission detail limit reached"))
        } else {
            Ok(bytes.len())
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::schema::v1::{Diff, ToolCallLocation};

    #[test]
    fn includes_actual_command_paths_text_and_file_change() {
        let fields = ToolCallUpdateFields::new()
            .raw_input(serde_json::json!({"command":"cargo check","cwd":"C:/project"}))
            .locations(vec![ToolCallLocation::new("C:/project/Cargo.toml").line(4)])
            .content(vec![
                "Check the project".into(),
                Diff::new("C:/project/example.txt", "new content")
                    .old_text("old content")
                    .into(),
            ]);
        let details = describe(&fields).unwrap();
        for expected in [
            "cargo check",
            "C:/project/Cargo.toml:4",
            "Check the project",
            "Before:\nold content",
            "After:\nnew content",
        ] {
            assert!(details.contains(expected), "missing {expected}");
        }
    }

    #[test]
    fn omitted_or_empty_details_stay_absent() {
        assert_eq!(describe(&ToolCallUpdateFields::new()), None);
        let fields = ToolCallUpdateFields::new()
            .raw_input(serde_json::json!({}))
            .content(vec![" \n".into()])
            .locations(vec![]);
        assert_eq!(describe(&fields), None);
    }

    #[test]
    fn text_and_json_are_bounded_with_visible_utf8_safe_truncation() {
        let large = "汉🙂".repeat(12_000);
        for fields in [
            ToolCallUpdateFields::new().content(vec![large.clone().into()]),
            ToolCallUpdateFields::new().raw_input(serde_json::json!({"command":large})),
        ] {
            let details = describe(&fields).unwrap();
            assert!(details.len() <= MAX_BYTES);
            assert!(details.ends_with(TRUNCATED));
            assert!(details.contains("汉🙂"));
            assert!(!details.contains('\u{fffd}'));
        }
    }
}
