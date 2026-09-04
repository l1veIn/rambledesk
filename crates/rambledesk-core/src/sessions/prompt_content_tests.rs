use super::*;

const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jP1sAAAAASUVORK5CYII=";
fn image(data: impl Into<String>) -> SessionPromptContent {
    SessionPromptContent::Image {
        mime_type: "image/png".into(),
        data: data.into(),
    }
}
fn big_image(decoded_len: usize) -> SessionPromptContent {
    let mut bytes = vec![0; decoded_len];
    bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    image(base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[test]
fn typed_input_keeps_order_and_rejects_invalid_or_oversized_payloads() {
    let input = SendManagedPromptContentInput {
        session_id: "one".into(),
        text: "Inspect".into(),
        content: vec![image(PNG)],
    };
    let blocks = input.into_blocks().unwrap();
    assert!(matches!(&blocks[0], SessionPromptContent::Text { text } if text == "Inspect"));
    assert_eq!(blocks[1], image(PNG));
    assert!(validate_prompt_content(&[image("not base64")]).is_err());
    assert!(
        validate_prompt_content(&[image(
            base64::engine::general_purpose::STANDARD.encode(b"not a PNG")
        )])
        .is_err()
    );
    assert!(
        validate_prompt_content(&[SessionPromptContent::Image {
            mime_type: "image/svg+xml".into(),
            data: PNG.into()
        }])
        .is_err()
    );
    assert!(validate_prompt_content(&[big_image(MAX_PROMPT_IMAGE_BASE64_BYTES / 4 * 3)]).is_ok());
    assert!(
        validate_prompt_content(&[big_image(MAX_PROMPT_IMAGE_BASE64_BYTES / 4 * 3 + 1)]).is_err()
    );
    assert!(
        validate_prompt_content(&[
            big_image(1_100_000),
            big_image(1_100_000),
            big_image(1_100_000)
        ])
        .is_err()
    );
    assert!(
        validate_prompt_content(&vec![
            SessionPromptContent::Text { text: "x".into() };
            MAX_PROMPT_CONTENT_BLOCKS + 1
        ])
        .is_err()
    );
    assert!(
        validate_prompt_content(&[SessionPromptContent::Text {
            text: "x".repeat(MAX_PROMPT_TEXT_BYTES + 1)
        }])
        .is_err()
    );
    assert!(validate_prompt_content(&[SessionPromptContent::Text { text: "  ".into() }]).is_err());
}

#[test]
fn resources_have_scheme_and_size_bounds_and_real_capability_gates() {
    let link = |uri: &str| SessionPromptContent::ResourceLink {
        uri: uri.into(),
        name: "Reference".into(),
        mime_type: None,
    };
    for uri in [
        "file:///definitely-not-read.txt",
        "https://example.test/context",
        "http://localhost/context",
    ] {
        assert!(validate_prompt_content(&[link(uri)]).is_ok());
    }
    for uri in [
        "javascript://alert(1)",
        "data://payload",
        "C:\\secret.txt",
        "file:///line\nname",
        "https://",
        "https://a b",
    ] {
        assert!(validate_prompt_content(&[link(uri)]).is_err());
    }
    let resource = SessionPromptContent::Resource {
        uri: "file:///context.md".into(),
        mime_type: Some("text/markdown".into()),
        text: "Context".into(),
    };
    assert!(!prompt_content_supported(
        &[image(PNG)],
        &AgentPromptCapabilities::default()
    ));
    assert!(!prompt_content_supported(
        std::slice::from_ref(&resource),
        &AgentPromptCapabilities::default()
    ));
    let caps = AgentPromptCapabilities {
        image: true,
        audio: false,
        embedded_context: true,
        resource_links: true,
    };
    assert!(prompt_content_supported(
        &[image(PNG), resource, link("file:///context.md")],
        &caps
    ));
}

#[test]
fn large_input_is_sent_whole_but_display_preview_omits_media_and_preserves_text() {
    let blocks = vec![
        big_image(400_000),
        SessionPromptContent::Text {
            text: "Explain the image".into(),
        },
    ];
    validate_prompt_content(&blocks).unwrap();
    let preview = prompt_display(&blocks);
    assert!(preview.valid_size());
    let SessionActivityContent::Message {
        blocks: display,
        truncated,
    } = preview
    else {
        panic!("message")
    };
    assert!(truncated);
    assert!(matches!(
        display[0],
        SessionContentBlock::Image { data: None, .. }
    ));
    assert!(
        matches!(&display[1], SessionContentBlock::Text { text } if text == "Explain the image")
    );
    assert!(
        matches!(&blocks[0], SessionPromptContent::Image { data, .. } if data.len() > MAX_INLINE_MEDIA_BASE64_BYTES)
    );
}

#[test]
fn browser_attachment_uri_is_scoped_to_embedded_resources_and_a_valid_filename() {
    let authority = "019914a1-79d9-7a5d-8306-58f6ce62daa1";
    let valid = format!("ramble-attachment://{authority}/%E6%96%87%E6%A1%A3%20notes.md");
    let resource = |uri: String| SessionPromptContent::Resource {
        uri,
        mime_type: Some("text/markdown".into()),
        text: "Uploaded body".into(),
    };
    assert!(validate_prompt_content(&[resource(valid.clone())]).is_ok());
    assert!(
        validate_prompt_content(&[SessionPromptContent::ResourceLink {
            uri: valid,
            name: "Notes".into(),
            mime_type: None
        }])
        .is_err()
    );
    for invalid in [
        "ramble-attachment://not-a-uuid/notes.md".to_string(),
        format!("ramble-attachment://user@{authority}/notes.md"),
        format!("ramble-attachment://{authority}/"),
        format!("ramble-attachment://{authority}/notes.md?x=1"),
        format!("ramble-attachment://{authority}/notes.md#part"),
        format!("ramble-attachment://{authority}/a%2Fb"),
        format!("ramble-attachment://{authority}/%00"),
        format!("ramble-attachment://{authority}/%zz"),
    ] {
        assert!(validate_prompt_content(&[resource(invalid)]).is_err());
    }
}
