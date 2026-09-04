use super::*;
use serde_json::json;

fn tool(value: serde_json::Value, initial: bool) -> SessionToolCallPatch {
    let update = if initial {
        acp::SessionUpdate::ToolCall(serde_json::from_value(value).unwrap())
    } else {
        acp::SessionUpdate::ToolCallUpdate(serde_json::from_value(value).unwrap())
    };
    match convert(update).unwrap().unwrap() {
        AgentSessionEvent::ToolCall { patch, .. } => patch,
        _ => panic!("tool event"),
    }
}

#[test]
fn field_patches_preserve_raw_values_and_replace_collections() {
    let mut state = SessionToolCall::new("tool".into());
    state.apply_patch(tool(
        json!({
            "toolCallId":"tool", "title":"Read file", "kind":"read", "status":"in_progress",
            "rawInput":{"path":"/project/a"}, "rawOutput":{"ok":false},
            "content":[{"type":"diff","path":"/project/a","oldText":"before","newText":"after"}],
            "locations":[{"path":"/project/a","line":4}], "_meta":{"toolName":"read_file"}
        }),
        true,
    ));
    let initial = state.clone();
    state.apply_patch(tool(
        json!({"toolCallId":"tool", "status":"completed"}),
        false,
    ));
    assert_eq!(state.status, SessionToolStatus::Completed);
    assert_eq!(state.title, "Read file");
    assert_eq!(state.name.as_deref(), Some("read_file"));
    assert_eq!(state.raw_input, initial.raw_input);
    assert_eq!(state.raw_output, initial.raw_output);
    assert_eq!(state.content, initial.content);
    assert_eq!(state.locations, initial.locations);
    state.apply_patch(tool(
        json!({"toolCallId":"tool", "content":[], "locations":[], "rawOutput":{"ok":true}}),
        false,
    ));
    assert!(state.content.is_empty() && state.locations.is_empty());
    assert_eq!(state.raw_output.as_deref(), Some(r#"{"ok":true}"#));
    assert_eq!(state.raw_input, initial.raw_input);
}

#[test]
fn oversized_utf8_raw_and_media_are_bounded_and_visibly_truncated() {
    let patch = tool(
        json!({"toolCallId":"tool", "rawOutput":{"text":"字".repeat(100_000)}}),
        false,
    );
    assert!(patch.truncated);
    assert!(patch.raw_output.unwrap().len() <= MAX_TOOL_RAW_BYTES);
    let image = acp::ContentBlock::Image(acp::ImageContent::new(
        "a".repeat(MAX_INLINE_MEDIA_BASE64_BYTES + 4),
        "image/png",
    ));
    match message(SessionActivityKind::AgentMessage, &image) {
        AgentSessionEvent::MessageChunk {
            block: SessionContentBlock::Image {
                data, mime_type, ..
            },
            truncated,
            ..
        } => {
            assert!(truncated);
            assert!(data.is_none());
            assert_eq!(mime_type, "image/png");
        }
        _ => panic!("image preserved"),
    }
}

#[test]
fn supported_nontext_content_and_tool_locations_survive_normalization() {
    let content: acp::ContentBlock = serde_json::from_value(json!({"type":"resource", "resource":{
        "uri":"file:///project/context", "mimeType":"text/plain", "text":"Context body"
    }}))
    .unwrap();
    match message(SessionActivityKind::AgentThought, &content) {
        AgentSessionEvent::MessageChunk {
            kind: SessionActivityKind::AgentThought,
            block: SessionContentBlock::Resource { uri, text, .. },
            truncated,
        } => {
            assert_eq!(uri, "file:///project/context");
            assert_eq!(text.as_deref(), Some("Context body"));
            assert!(!truncated);
        }
        _ => panic!("resource preserved"),
    }
    let mut state = SessionToolCall::new("tool".into());
    state.apply_patch(tool(
        json!({"toolCallId":"tool", "title":"Run", "kind":"execute", "content":[
            {"type":"terminal","terminalId":"terminal-one"},
            {"type":"content","content":{"type":"text","text":"Output"}}
        ]}),
        true,
    ));
    assert!(
        matches!(&state.content[0], SessionContentBlock::Terminal { terminal_id } if terminal_id == "terminal-one")
    );
    assert!(SessionActivityContent::ToolCall { tool: state }.valid_size());
}

#[test]
fn maximum_tool_payloads_fit_persistent_content_limit() {
    let mut state = SessionToolCall::new("tool".into());
    state.apply_patch(tool(
        json!({"toolCallId":"tool", "title":"a".repeat(5000),
            "content":[{"type":"content","content":{"type":"text","text":"x".repeat(600_000)}}],
            "rawInput":{"x":"a".repeat(100_000)}, "rawOutput":{"x":"b".repeat(100_000)},
            "locations":(0..100).map(|_|json!({"path":"p".repeat(2000)})).collect::<Vec<_>>()
        }),
        true,
    ));
    assert!(state.truncated);
    assert!(SessionActivityContent::ToolCall { tool: state }.valid_size());
}
