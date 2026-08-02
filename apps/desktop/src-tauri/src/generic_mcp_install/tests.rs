use super::*;
use serde_json::json;

fn configuration() -> String {
    json!({
        "mcpServers": {
            "rambledesk": {
                "type": "http",
                "url": "http://127.0.0.1:37642/mcp",
                "headers": { "Authorization": "Bearer test-token" }
            }
        }
    })
    .to_string()
}

#[test]
fn json_install_preserves_sibling_servers_and_is_idempotent() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("mcp.json");
    fs::write(
        &path,
        r#"{"mcpServers":{"other":{"command":"other"}},"theme":"dark"}"#,
    )
    .expect("seed config");
    let entry = entry_for_host(
        &extract_server_entry(&configuration()).expect("entry"),
        "claude",
    )
    .expect("host entry");
    assert_eq!(
        write_json_config(&path, entry.clone()).expect("install"),
        "updated"
    );
    assert_eq!(
        write_json_config(&path, entry).expect("repeat"),
        "unchanged"
    );
    let written: Value =
        serde_json::from_str(&fs::read_to_string(path).expect("read")).expect("valid json");
    assert_eq!(written["theme"], "dark");
    assert_eq!(written["mcpServers"]["other"]["command"], "other");
    assert_eq!(written["mcpServers"][SERVER_ID]["type"], "http");
    assert_eq!(
        written["mcpServers"][SERVER_ID]["headers"][HOST_HEADER],
        "claude"
    );
    assert_eq!(
        written["mcpServers"][SERVER_ID]["env"][HOST_ENV_KEY],
        "claude"
    );
}

#[test]
fn codex_install_preserves_unrelated_toml_and_omits_stdio_env() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("config.toml");
    fs::write(
        &path,
        "model = \"gpt-5\"\n\n[mcp_servers.other]\ncommand = \"other\"\n",
    )
    .expect("seed config");
    let entry = entry_for_host(
        &extract_server_entry(&configuration()).expect("entry"),
        "codex",
    )
    .expect("host entry");
    assert_eq!(
        write_codex_config(&path, &entry).expect("install"),
        "updated"
    );
    assert_eq!(
        write_codex_config(&path, &entry).expect("repeat"),
        "unchanged"
    );
    let written = fs::read_to_string(path).expect("read");
    assert!(written.contains("model = \"gpt-5\""));
    assert!(written.contains("[mcp_servers.other]"));
    assert!(written.contains("[mcp_servers.rambledesk]"));
    assert!(written.contains("[mcp_servers.rambledesk.http_headers]"));
    assert!(written.contains("x-rambledesk-host"));
    assert!(written.contains("codex"));
    assert!(!written.contains("[mcp_servers.rambledesk.env]"));
    assert!(!written.contains("RAMBLEDESK_HOST"));
}

#[test]
fn codex_install_repairs_legacy_streamable_http_env() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("config.toml");
    fs::write(
        &path,
        concat!(
            "model = \"gpt-5\"\n\n",
            "[mcp_servers.rambledesk]\n",
            "url = \"http://127.0.0.1:37642/mcp\"\n\n",
            "[mcp_servers.rambledesk.env]\n",
            "RAMBLEDESK_HOST = \"codex\"\n",
        ),
    )
    .expect("seed legacy config");
    assert!(!codex_is_configured(&path));
    let entry = entry_for_host(
        &extract_server_entry(&configuration()).expect("entry"),
        "codex",
    )
    .expect("host entry");

    assert_eq!(
        write_codex_config(&path, &entry).expect("repair"),
        "updated"
    );
    let written = fs::read_to_string(&path).expect("read");
    assert!(written.contains("model = \"gpt-5\""));
    assert!(written.contains("[mcp_servers.rambledesk.http_headers]"));
    assert!(!written.contains("[mcp_servers.rambledesk.env]"));
    assert!(!written.contains("RAMBLEDESK_HOST"));
    assert!(codex_is_configured(&path));
}

#[test]
fn opencode_install_preserves_sibling_servers_and_uses_remote_shape() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("opencode.json");
    fs::write(
            &path,
            r#"{"mcp":{"codegraph":{"type":"local","command":["codegraph","serve","--mcp"]}},"theme":"system"}"#,
        )
        .expect("seed config");
    let entry = entry_for_host(
        &extract_server_entry(&configuration()).expect("entry"),
        "opencode",
    )
    .expect("host entry");
    assert_eq!(
        write_opencode_config(&path, &entry).expect("install"),
        "updated"
    );
    assert_eq!(
        write_opencode_config(&path, &entry).expect("repeat"),
        "unchanged"
    );
    let written: Value =
        serde_json::from_str(&fs::read_to_string(path).expect("read")).expect("valid json");
    assert_eq!(written["theme"], "system");
    assert_eq!(written["mcp"]["codegraph"]["type"], "local");
    assert_eq!(written["mcp"][SERVER_ID]["type"], "remote");
    assert_eq!(written["mcp"][SERVER_ID]["enabled"], true);
    assert_eq!(
        written["mcp"][SERVER_ID]["headers"]["Authorization"],
        "Bearer test-token"
    );
    assert_eq!(
        written["mcp"][SERVER_ID]["headers"][HOST_HEADER],
        "opencode"
    );
}

#[test]
fn invalid_existing_config_is_never_overwritten() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("mcp.json");
    fs::write(&path, "{ invalid").expect("seed config");
    let entry = extract_server_entry(&configuration()).expect("entry");
    let error = write_json_config(&path, entry).expect_err("invalid config must fail");
    assert!(error.contains("Refusing to overwrite invalid JSON"));
    assert_eq!(fs::read_to_string(path).expect("read"), "{ invalid");
}
