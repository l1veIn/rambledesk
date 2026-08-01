use serde::Serialize;
use serde_json::{Map, Value, json};
use std::{
    env, fs,
    path::{Path, PathBuf},
};
use toml_edit::{DocumentMut, Item, Table, value};

const SERVER_ID: &str = "rambledesk";
const HOST_ENV_KEY: &str = rambledesk_mcp::HOST_ENV_KEY;
const HOST_HEADER: &str = "X-RambleDesk-Host";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpClientView {
    pub id: &'static str,
    pub name: &'static str,
    pub installed: bool,
    pub configured: bool,
    pub config_path: String,
    pub restart_required: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInstallResult {
    pub client_id: String,
    pub action: &'static str,
    pub config_path: String,
    pub restart_required: bool,
}

#[derive(Clone, Copy)]
enum ClientKind {
    Claude,
    Codex,
    Cursor,
    Gemini,
}

impl ClientKind {
    const ALL: [Self; 4] = [Self::Claude, Self::Codex, Self::Cursor, Self::Gemini];

    fn id(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Cursor => "cursor",
            Self::Gemini => "gemini",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
            Self::Cursor => "Cursor",
            Self::Gemini => "Gemini CLI",
        }
    }

    fn executable(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Cursor => "cursor",
            Self::Gemini => "gemini",
        }
    }

    fn config_path(self, home: &Path) -> PathBuf {
        match self {
            Self::Claude => home.join(".claude.json"),
            Self::Codex => env::var_os("CODEX_HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".codex"))
                .join("config.toml"),
            Self::Cursor => home.join(".cursor").join("mcp.json"),
            Self::Gemini => home.join(".gemini").join("settings.json"),
        }
    }

    fn marker_path(self, home: &Path) -> PathBuf {
        match self {
            Self::Claude => home.join(".claude"),
            Self::Codex => self
                .config_path(home)
                .parent()
                .unwrap_or(home)
                .to_path_buf(),
            Self::Cursor => home.join(".cursor"),
            Self::Gemini => home.join(".gemini"),
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|client| client.id() == id)
    }
}

pub fn detect_clients(home: &Path) -> Vec<McpClientView> {
    ClientKind::ALL
        .into_iter()
        .map(|client| {
            let config_path = client.config_path(home);
            let configured = match client {
                ClientKind::Codex => codex_is_configured(&config_path),
                _ => json_is_configured(&config_path),
            };
            McpClientView {
                id: client.id(),
                name: client.name(),
                installed: client.marker_path(home).exists()
                    || executable_on_path(client.executable()),
                configured,
                config_path: config_path.to_string_lossy().into_owned(),
                restart_required: true,
            }
        })
        .collect()
}

pub fn install_clients(
    home: &Path,
    client_ids: &[String],
    mcp_configuration: &str,
) -> Result<Vec<McpInstallResult>, String> {
    if client_ids.is_empty() {
        return Err("Select at least one detected coding tool".to_owned());
    }
    let base_entry = extract_server_entry(mcp_configuration)?;
    let detected = detect_clients(home);
    let mut results = Vec::with_capacity(client_ids.len());

    for id in client_ids {
        let client =
            ClientKind::from_id(id).ok_or_else(|| format!("Unsupported MCP client: {id}"))?;
        let view = detected
            .iter()
            .find(|candidate| candidate.id == client.id())
            .expect("all supported clients are detected");
        if !view.installed {
            return Err(format!("{} was not detected on this device", client.name()));
        }
        let path = client.config_path(home);
        let entry = entry_for_host(&base_entry, client.id())?;
        let action = match client {
            ClientKind::Codex => write_codex_config(&path, &entry)?,
            ClientKind::Gemini => {
                let mut entry = entry;
                if let Some(object) = entry.as_object_mut() {
                    object.remove("type");
                    if let Some(url) = object.remove("url") {
                        object.insert("httpUrl".to_owned(), url);
                    }
                }
                write_json_config(&path, entry)?
            }
            ClientKind::Claude | ClientKind::Cursor => write_json_config(&path, entry)?,
        };
        results.push(McpInstallResult {
            client_id: id.clone(),
            action,
            config_path: path.to_string_lossy().into_owned(),
            restart_required: true,
        });
    }
    Ok(results)
}

fn extract_server_entry(configuration: &str) -> Result<Value, String> {
    let parsed: Value = serde_json::from_str(configuration)
        .map_err(|error| format!("Invalid RambleDesk MCP configuration: {error}"))?;
    parsed
        .get("mcpServers")
        .and_then(|servers| servers.get(SERVER_ID))
        .cloned()
        .ok_or_else(|| "RambleDesk MCP configuration is missing its server entry".to_owned())
}

/// Stamp install-time host identity onto a shared base MCP entry.
fn entry_for_host(base_entry: &Value, host_id: &str) -> Result<Value, String> {
    let mut entry = base_entry.clone();
    let object = entry
        .as_object_mut()
        .ok_or_else(|| "RambleDesk MCP server entry must be a JSON object".to_owned())?;

    let headers = object
        .entry("headers")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| "RambleDesk MCP headers must be a JSON object".to_owned())?;
    headers.insert(HOST_HEADER.to_owned(), Value::String(host_id.to_owned()));

    object.insert("env".to_owned(), json!({ HOST_ENV_KEY: host_id }));
    Ok(entry)
}

fn json_is_configured(path: &Path) -> bool {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .and_then(|value| value.get("mcpServers")?.get(SERVER_ID).cloned())
        .is_some()
}

fn codex_is_configured(path: &Path) -> bool {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| content.parse::<DocumentMut>().ok())
        .and_then(|document| {
            document
                .get("mcp_servers")?
                .get(SERVER_ID)
                .and_then(Item::as_table)
                .map(|_| ())
        })
        .is_some()
}

fn write_json_config(path: &Path, entry: Value) -> Result<&'static str, String> {
    let existed = path.exists();
    let mut root = if existed {
        let content = fs::read_to_string(path)
            .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
        serde_json::from_str::<Value>(&content).map_err(|error| {
            format!(
                "Refusing to overwrite invalid JSON at {}: {error}",
                path.display()
            )
        })?
    } else {
        Value::Object(Map::new())
    };
    let root_object = root
        .as_object_mut()
        .ok_or_else(|| format!("{} must contain a JSON object", path.display()))?;
    let servers = root_object
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| format!("mcpServers in {} must be a JSON object", path.display()))?;
    let unchanged = servers.get(SERVER_ID) == Some(&entry);
    if unchanged {
        return Ok("unchanged");
    }
    servers.insert(SERVER_ID.to_owned(), entry);
    let content = serde_json::to_string_pretty(&root)
        .map_err(|error| format!("Could not serialize MCP configuration: {error}"))?
        + "\n";
    write_config(path, content.as_bytes())?;
    Ok(if existed { "updated" } else { "created" })
}

fn write_codex_config(path: &Path, entry: &Value) -> Result<&'static str, String> {
    let url = entry
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| "RambleDesk MCP URL is missing".to_owned())?;
    let authorization = entry
        .get("headers")
        .and_then(|headers| headers.get("Authorization"))
        .and_then(Value::as_str)
        .ok_or_else(|| "RambleDesk MCP authorization header is missing".to_owned())?;
    let host_id = entry
        .get("headers")
        .and_then(|headers| headers.get(HOST_HEADER))
        .and_then(Value::as_str)
        .or_else(|| {
            entry
                .get("env")
                .and_then(|env| env.get(HOST_ENV_KEY))
                .and_then(Value::as_str)
        });
    let existed = path.exists();
    let source = if existed {
        fs::read_to_string(path)
            .map_err(|error| format!("Could not read {}: {error}", path.display()))?
    } else {
        String::new()
    };
    let mut document = if source.trim().is_empty() {
        DocumentMut::new()
    } else {
        source.parse::<DocumentMut>().map_err(|error| {
            format!(
                "Refusing to overwrite invalid TOML at {}: {error}",
                path.display()
            )
        })?
    };
    let mut server = Table::new();
    server["url"] = value(url);
    let mut headers = Table::new();
    headers["Authorization"] = value(authorization);
    if let Some(host_id) = host_id {
        headers[HOST_HEADER] = value(host_id);
        let mut env_table = Table::new();
        env_table[HOST_ENV_KEY] = value(host_id);
        server["env"] = Item::Table(env_table);
    }
    server["http_headers"] = Item::Table(headers);
    let before = document.to_string();
    document["mcp_servers"][SERVER_ID] = Item::Table(server);
    let after = document.to_string();
    if before == after {
        return Ok("unchanged");
    }
    write_config(path, after.as_bytes())?;
    Ok(if existed { "updated" } else { "created" })
}

fn write_config(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    fs::write(path, contents)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))
}

fn executable_on_path(name: &str) -> bool {
    let Some(path_value) = env::var_os("PATH") else {
        return false;
    };
    #[cfg(windows)]
    let candidates = [
        name.to_owned(),
        format!("{name}.exe"),
        format!("{name}.cmd"),
        format!("{name}.bat"),
    ];
    #[cfg(not(windows))]
    let candidates = [name.to_owned()];
    env::split_paths(&path_value).any(|directory| {
        candidates
            .iter()
            .any(|candidate| directory.join(candidate).is_file())
    })
}

#[cfg(test)]
mod tests {
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
    fn codex_install_preserves_unrelated_toml() {
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
        assert!(written.contains("RAMBLEDESK_HOST"));
        assert!(written.contains("X-RambleDesk-Host"));
        assert!(written.contains("codex"));
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
}
