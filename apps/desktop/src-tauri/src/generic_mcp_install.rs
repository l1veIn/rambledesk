use serde::Serialize;
use serde_json::{Map, Value, json};
use std::{
    env, fs,
    path::{Path, PathBuf},
};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

use rambledesk_hosts::{HostProfile, host_profile};

use crate::platform::process::find_executable;

const SERVER_ID: &str = "rambledesk";
const HOST_ENV_KEY: &str = rambledesk_local_server::HOST_ENV_KEY;
const HOST_HEADER: &str = rambledesk_local_server::HOST_HEADER;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpHostView {
    pub id: &'static str,
    pub name: String,
    pub icon_svg: String,
    pub installed: bool,
    pub configured: bool,
    pub config_path: String,
    pub restart_required: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInstallResult {
    pub host_id: String,
    pub action: &'static str,
    pub config_path: String,
    pub restart_required: bool,
}

#[derive(Clone, Copy)]
enum HostKind {
    Claude,
    Codex,
    Cursor,
    Gemini,
    OpenCode,
    Reasonix,
}

impl HostKind {
    const ALL: [Self; 6] = [
        Self::Claude,
        Self::Codex,
        Self::Cursor,
        Self::Gemini,
        Self::OpenCode,
        Self::Reasonix,
    ];

    fn id(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Cursor => "cursor",
            Self::Gemini => "gemini",
            Self::OpenCode => "opencode",
            Self::Reasonix => "reasonix",
        }
    }

    fn profile(self) -> HostProfile {
        host_profile(self.id())
    }

    fn name(self) -> String {
        self.profile().label
    }

    fn executable(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Cursor => "cursor",
            Self::Gemini => "gemini",
            Self::OpenCode => "opencode",
            Self::Reasonix => "reasonix",
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
            Self::OpenCode => home.join(".config").join("opencode").join("opencode.json"),
            Self::Reasonix => reasonix_home(home).join("config.toml"),
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
            Self::OpenCode => home.join(".opencode"),
            Self::Reasonix => reasonix_home(home),
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|host| host.id() == id)
    }
}

pub fn detect_hosts(home: &Path) -> Vec<McpHostView> {
    HostKind::ALL
        .into_iter()
        .map(|host| {
            let config_path = host.config_path(home);
            let profile = host.profile();
            let configured = match host {
                HostKind::Codex => codex_is_configured(&config_path),
                HostKind::OpenCode => opencode_is_configured(&config_path),
                HostKind::Reasonix => reasonix_is_configured(&config_path),
                _ => json_is_configured(&config_path),
            };
            McpHostView {
                id: host.id(),
                name: profile.label,
                icon_svg: profile.icon_svg,
                installed: host.marker_path(home).exists()
                    || find_executable(host.executable()).is_some(),
                configured,
                config_path: config_path.to_string_lossy().into_owned(),
                restart_required: true,
            }
        })
        .collect()
}

pub fn install_hosts(
    home: &Path,
    host_ids: &[String],
    mcp_configuration: &str,
) -> Result<Vec<McpInstallResult>, String> {
    if host_ids.is_empty() {
        return Err("Select at least one detected coding tool".to_owned());
    }
    let base_entry = extract_server_entry(mcp_configuration)?;
    let detected = detect_hosts(home);
    let mut results = Vec::with_capacity(host_ids.len());

    for id in host_ids {
        let host = HostKind::from_id(id).ok_or_else(|| format!("Unsupported host: {id}"))?;
        let view = detected
            .iter()
            .find(|candidate| candidate.id == host.id())
            .expect("all supported hosts are detected");
        if !view.installed {
            return Err(format!("{} was not detected on this device", host.name()));
        }
        let path = host.config_path(home);
        let entry = entry_for_host(&base_entry, host.id())?;
        let action = match host {
            HostKind::Codex => write_codex_config(&path, &entry)?,
            HostKind::OpenCode => write_opencode_config(&path, &entry)?,
            HostKind::Reasonix => write_reasonix_config(&path, &entry)?,
            HostKind::Gemini => {
                let mut entry = entry;
                if let Some(object) = entry.as_object_mut() {
                    object.remove("type");
                    if let Some(url) = object.remove("url") {
                        object.insert("httpUrl".to_owned(), url);
                    }
                }
                write_json_config(&path, entry)?
            }
            HostKind::Claude | HostKind::Cursor => write_json_config(&path, entry)?,
        };
        results.push(McpInstallResult {
            host_id: id.clone(),
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
        .ok_or_else(|| "Generic MCP Adapter entry must be a JSON object".to_owned())?;

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
                .filter(|server| !server.contains_key("env"))
                .map(|_| ())
        })
        .is_some()
}

fn opencode_is_configured(path: &Path) -> bool {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .and_then(|value| value.get("mcp")?.get(SERVER_ID).cloned())
        .is_some()
}

/// Reasonix home directory. `REASONIX_HOME` overrides for portable installs;
/// otherwise Windows uses `%APPDATA%\reasonix` and macOS/Linux use `~/.reasonix`.
fn reasonix_home(home: &Path) -> PathBuf {
    if let Some(override_home) = env::var_os("REASONIX_HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(override_home);
    }
    #[cfg(windows)]
    {
        if let Some(appdata) = env::var_os("APPDATA").filter(|value| !value.is_empty()) {
            return PathBuf::from(appdata).join("reasonix");
        }
        home.join("AppData").join("Roaming").join("reasonix")
    }
    #[cfg(not(windows))]
    {
        home.join(".reasonix")
    }
}

fn reasonix_is_configured(path: &Path) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(document) = content.parse::<DocumentMut>() else {
        return false;
    };
    document
        .get("plugins")
        .and_then(Item::as_array_of_tables)
        .is_some_and(|plugins| {
            plugins
                .iter()
                .any(|plugin| plugin.get("name").and_then(Item::as_str) == Some(SERVER_ID))
        })
}

fn write_reasonix_config(path: &Path, entry: &Value) -> Result<&'static str, String> {
    let url = entry
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| "RambleDesk MCP URL is missing".to_owned())?;
    let headers = entry
        .get("headers")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| "RambleDesk MCP headers are missing".to_owned())?;
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
    server["name"] = value(SERVER_ID);
    server["type"] = value("http");
    server["url"] = value(url);
    let mut header_table = Table::new();
    for (key, header_value) in &headers {
        let text = header_value
            .as_str()
            .ok_or_else(|| format!("RambleDesk MCP header {key} must be a string"))?;
        header_table[&**key] = value(text);
    }
    server["headers"] = Item::Table(header_table);

    let plugins = document
        .entry("plugins")
        .or_insert(Item::ArrayOfTables(ArrayOfTables::new()));
    let Some(array) = plugins.as_array_of_tables_mut() else {
        return Err(format!(
            "plugins in {} must be an array of tables",
            path.display()
        ));
    };
    let mut replaced = false;
    for table in array.iter_mut() {
        if table.get("name").and_then(Item::as_str) == Some(SERVER_ID) {
            *table = server.clone();
            replaced = true;
            break;
        }
    }
    if !replaced {
        array.push(server);
    }
    let after = document.to_string();
    if after == source {
        return Ok("unchanged");
    }
    write_config(path, after.as_bytes())?;
    Ok(if existed { "updated" } else { "created" })
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

fn write_opencode_config(path: &Path, entry: &Value) -> Result<&'static str, String> {
    let url = entry
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| "RambleDesk MCP URL is missing".to_owned())?;
    let headers = entry
        .get("headers")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| "RambleDesk MCP headers are missing".to_owned())?;
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
        .entry("mcp")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| format!("mcp in {} must be a JSON object", path.display()))?;
    let opencode_entry = json!({
        "type": "remote",
        "url": url,
        "enabled": true,
        "headers": headers,
    });
    let unchanged = servers.get(SERVER_ID) == Some(&opencode_entry);
    if unchanged {
        return Ok("unchanged");
    }
    servers.insert(SERVER_ID.to_owned(), opencode_entry);
    let content = serde_json::to_string_pretty(&root)
        .map_err(|error| format!("Could not serialize OpenCode MCP configuration: {error}"))?
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
        .and_then(Value::as_str);
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

#[cfg(test)]
#[path = "generic_mcp_install/tests.rs"]
mod tests;
