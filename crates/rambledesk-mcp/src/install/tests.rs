use super::*;
use rambledesk_hosts::ConfigFormat;
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
fn gemini_format_reshapes_url_and_omits_type() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("settings.json");
    let entry = entry_for_host(
        &extract_server_entry(&configuration()).expect("entry"),
        "gemini",
    )
    .expect("host entry");
    assert_eq!(
        write_config_for(ConfigFormat::GeminiSettingsJson, &path, entry.clone()).expect("install"),
        "created"
    );
    assert_eq!(
        write_config_for(ConfigFormat::GeminiSettingsJson, &path, entry).expect("repeat"),
        "unchanged"
    );
    let written: Value =
        serde_json::from_str(&fs::read_to_string(path).expect("read")).expect("valid json");
    let server = &written["mcpServers"][SERVER_ID];
    assert_eq!(server["httpUrl"], "http://127.0.0.1:37642/mcp");
    assert!(server.get("url").is_none());
    assert!(server.get("type").is_none());
    assert_eq!(server["headers"][HOST_HEADER], "gemini");
}

#[test]
fn antigravity_format_reshapes_url_to_server_url_and_omits_type() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("mcp_config.json");
    let entry = entry_for_host(
        &extract_server_entry(&configuration()).expect("entry"),
        "antigravity",
    )
    .expect("host entry");
    assert_eq!(
        write_config_for(ConfigFormat::AntigravityMcpJson, &path, entry.clone()).expect("install"),
        "created"
    );
    assert_eq!(
        write_config_for(ConfigFormat::AntigravityMcpJson, &path, entry).expect("repeat"),
        "unchanged"
    );
    let written: Value =
        serde_json::from_str(&fs::read_to_string(path).expect("read")).expect("valid json");
    let server = &written["mcpServers"][SERVER_ID];
    assert_eq!(server["serverUrl"], "http://127.0.0.1:37642/mcp");
    assert!(server.get("url").is_none());
    assert!(server.get("httpUrl").is_none());
    assert!(server.get("type").is_none());
    assert!(server.get("env").is_none());
    assert_eq!(server["headers"][HOST_HEADER], "antigravity");
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
fn grok_install_preserves_unrelated_toml_and_uses_headers() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("config.toml");
    fs::write(
        &path,
        "default_model = \"grok-4.5\"\n\n[mcp_servers.other]\ncommand = \"other\"\n",
    )
    .expect("seed config");
    let entry = entry_for_host(
        &extract_server_entry(&configuration()).expect("entry"),
        "grok",
    )
    .expect("host entry");
    assert_eq!(
        write_grok_config(&path, &entry).expect("install"),
        "updated"
    );
    assert_eq!(
        write_grok_config(&path, &entry).expect("repeat"),
        "unchanged"
    );
    let written = fs::read_to_string(path).expect("read");
    assert!(written.contains("default_model = \"grok-4.5\""));
    assert!(written.contains("[mcp_servers.other]"));
    assert!(written.contains("[mcp_servers.rambledesk]"));
    assert!(written.contains("[mcp_servers.rambledesk.headers]"));
    assert!(!written.contains("[mcp_servers.rambledesk.http_headers]"));
    assert!(written.contains("x-rambledesk-host"));
    assert!(written.contains("grok"));
    assert!(!written.contains("[mcp_servers.rambledesk.env]"));
    assert!(!written.contains("RAMBLEDESK_HOST"));
}

#[test]
fn grok_install_creates_config_and_is_detected() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("config.toml");
    let entry = entry_for_host(
        &extract_server_entry(&configuration()).expect("entry"),
        "grok",
    )
    .expect("host entry");
    assert_eq!(write_grok_config(&path, &entry).expect("create"), "created");
    let written = fs::read_to_string(&path).expect("read created config");
    assert!(
        written.contains("[mcp_servers.rambledesk]"),
        "expected standard mcp_servers table, got:\n{written}"
    );
    assert!(
        toml_mcp_servers_is_configured(&path, false),
        "expected grok mcp_servers.rambledesk table, got:\n{written}"
    );
}

#[test]
fn opencode_install_preserves_sibling_servers_and_uses_remote_shape() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("opencode.json");
    fs::write(
        &path,
        r#"{"mcp":{"other":{"type":"remote"}},"theme":"dark"}"#,
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
    assert_eq!(written["theme"], "dark");
    assert_eq!(written["mcp"]["other"]["type"], "remote");
    assert_eq!(written["mcp"][SERVER_ID]["type"], "remote");
    assert_eq!(
        written["mcp"][SERVER_ID]["url"],
        "http://127.0.0.1:37642/mcp"
    );
    assert_eq!(
        written["mcp"][SERVER_ID]["headers"][HOST_HEADER],
        "opencode"
    );
    assert!(written["mcp"][SERVER_ID].get("env").is_none());
}

#[test]
fn reasonix_install_preserves_sibling_plugins_and_is_idempotent() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("config.toml");
    fs::write(
        &path,
        "config_version = 1\n\n[[plugins]]\nname = \"codegraph\"\ntype = \"stdio\"\ncommand = \"npx\"\n",
    )
    .expect("seed config");
    let entry = entry_for_host(
        &extract_server_entry(&configuration()).expect("entry"),
        "reasonix",
    )
    .expect("host entry");
    assert_eq!(
        write_reasonix_config(&path, &entry).expect("install"),
        "updated"
    );
    assert_eq!(
        write_reasonix_config(&path, &entry).expect("repeat"),
        "unchanged"
    );
    let written = fs::read_to_string(&path).expect("read");
    assert!(written.contains("config_version = 1"));
    assert!(written.contains("name = \"codegraph\""));
    assert!(written.contains("name = \"rambledesk\""));
    assert!(written.contains("type = \"http\""));
    assert!(written.contains("url = \"http://127.0.0.1:37642/mcp\""));
    assert!(written.contains("x-rambledesk-host"));
    assert!(written.contains("reasonix"));
    assert!(!written.contains("RAMBLEDESK_HOST"));
    assert!(reasonix_is_configured(&path));
}

#[test]
fn reasonix_install_creates_config_and_replaces_existing_entry() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("config.toml");
    let entry = entry_for_host(
        &extract_server_entry(&configuration()).expect("entry"),
        "reasonix",
    )
    .expect("host entry");
    assert_eq!(
        write_reasonix_config(&path, &entry).expect("create"),
        "created"
    );
    assert!(reasonix_is_configured(&path));

    fs::write(
        &path,
        "[[plugins]]\nname = \"rambledesk\"\ntype = \"stdio\"\ncommand = \"stale\"\n",
    )
    .expect("seed stale entry");
    assert_eq!(
        write_reasonix_config(&path, &entry).expect("replace"),
        "updated"
    );
    let written = fs::read_to_string(&path).expect("read");
    assert!(written.contains("type = \"http\""));
    assert!(!written.contains("stale"));
    assert!(reasonix_is_configured(&path));
}

#[test]
fn empty_existing_json_is_initialized() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("mcp_config.json");
    fs::write(&path, "").expect("seed empty config");
    let entry = extract_server_entry(&configuration()).expect("entry");
    assert_eq!(
        write_json_config(&path, entry).expect("empty file is writable"),
        "updated"
    );
    let written: Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("valid json");
    assert!(written["mcpServers"][SERVER_ID].is_object());
}

#[test]
fn invalid_existing_config_is_never_overwritten() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("mcp.json");
    fs::write(&path, "{ invalid").expect("seed config");
    let entry = extract_server_entry(&configuration()).expect("entry");
    let error = write_json_config(&path, entry).expect_err("invalid config must fail");
    assert!(error.contains("Refusing to overwrite invalid JSON"));
    assert_eq!(fs::read_to_string(&path).expect("read"), "{ invalid");
}

#[test]
fn detect_marks_installed_via_marker_directory() {
    let directory = tempfile::tempdir().expect("temp dir");
    let home = directory.path();
    fs::create_dir_all(home.join(".claude")).expect("marker");
    fs::create_dir_all(home.join(".gemini").join("antigravity")).expect("antigravity");
    let views = detect_hosts(home);
    let antigravity = views
        .iter()
        .find(|view| view.id == "antigravity")
        .expect("antigravity");
    assert!(antigravity.installed);
    let claude = views
        .iter()
        .find(|view| view.id == "claude")
        .expect("claude");
    assert!(claude.installed);
    assert_eq!(
        views.iter().map(|view| view.id).collect::<Vec<_>>(),
        [
            "claude",
            "codex",
            "cursor",
            "gemini",
            "antigravity",
            "grok",
            "opencode",
            "reasonix"
        ]
    );
}

#[test]
fn ramble_skill_is_written_idempotently() {
    let directory = tempfile::tempdir().expect("temp dir");
    let skill_dir = directory.path().join(".claude").join("skills");
    assert_eq!(write_ramble_skill(&skill_dir).expect("install"), "created");
    let target = skill_dir.join("ramble").join("SKILL.md");
    assert_eq!(fs::read_to_string(&target).expect("read"), RAMBLE_SKILL_MD);
    assert_eq!(write_ramble_skill(&skill_dir).expect("repeat"), "unchanged");
}
