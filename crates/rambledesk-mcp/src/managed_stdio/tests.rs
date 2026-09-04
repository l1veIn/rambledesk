use super::*;
use tokio::io::AsyncReadExt;

#[test]
fn capability_is_only_literal_loopback_private_path_without_credentials_in_the_url() {
    let capability = |url: &str| ManagedFeedbackEndpoint {
        url: url.into(),
        bearer_token: "a".repeat(64),
    };
    for url in [
        "http://127.0.0.1:37642/mcp-managed",
        "http://[::1]:37642/mcp-managed",
    ] {
        validate(&capability(url)).unwrap();
    }
    for url in [
        "https://127.0.0.1/mcp-managed",
        "http://localhost/mcp-managed",
        "http://192.168.1.1/mcp-managed",
        "http://127.0.0.1/mcp",
        "http://127.0.0.1/mcp-managed?token=secret",
        "http://127.0.0.1/mcp-managed#token",
        "http://secret@127.0.0.1/mcp-managed",
        "http://127.0.0.1:0/mcp-managed",
    ] {
        assert_eq!(
            validate(&capability(url)),
            Err(ManagedStdioError::InvalidCapability)
        );
    }
    let mut invalid = capability("http://127.0.0.1:37642/mcp-managed");
    invalid.bearer_token = "fixture-secret\r\n".into();
    let error = validate(&invalid).unwrap_err();
    assert!(!format!("{error:?} {error}").contains("fixture-secret"));
}

#[tokio::test]
async fn incomplete_input_lines_are_bounded_and_multiple_small_lines_are_allowed() {
    let mut oversized =
        bounded_input::BoundedInput::new(std::io::Cursor::new(vec![b'x'; 2049]), 2048);
    assert_eq!(
        oversized.read_to_end(&mut vec![]).await.unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
    let bytes = b"1234\n1234\n1234\n";
    let mut input = bounded_input::BoundedInput::new(std::io::Cursor::new(bytes), 4);
    let mut output = vec![];
    input.read_to_end(&mut output).await.unwrap();
    assert_eq!(output, bytes);
}
