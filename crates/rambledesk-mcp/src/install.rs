//! Generic MCP Adapter client side: host detection and configuration install.
//!
//! This module executes against the host knowledge declared in
//! `rambledesk-hosts`: it discovers hosts on PATH / marker directories and
//! writes the RambleDesk server entry into each host's config file, dispatching
//! on the host's declared `ConfigFormat`. It contains no per-host knowledge of
//! its own — adding a host is a `rambledesk-hosts` change only.

use serde::Serialize;
use serde_json::{Map, Value, json};
use std::{fs, path::Path};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

use rambledesk_core::{HOST_ENV_KEY, HOST_HEADER, find_executable};
use rambledesk_hosts::{ConfigFormat, RAMBLE_SKILL_MD, generic_mcp_hosts, host_profile};

const SERVER_ID: &str = "rambledesk";

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

pub fn detect_hosts(home: &Path) -> Vec<McpHostView> {
    generic_mcp_hosts()
        .map(|knowledge| {
            let config_path = knowledge
                .config_path(home)
                .expect("generic MCP hosts declare a config path");
            let marker_path = knowledge
                .marker_path(home)
                .expect("generic MCP hosts declare a marker path");
            let profile = host_profile(knowledge.id);
            McpHostView {
                id: knowledge.id,
                name: profile.label,
                icon_svg: profile.icon_svg,
                installed: marker_path.exists()
                    || knowledge.executable.and_then(find_executable).is_some(),
                configured: is_configured(
                    knowledge
                        .config_format
                        .expect("generic MCP hosts declare a format"),
                    &config_path,
                ),
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
        let knowledge = generic_mcp_hosts()
            .find(|host| host.id == id)
            .ok_or_else(|| format!("Unsupported host: {id}"))?;
        let view = detected
            .iter()
            .find(|candidate| candidate.id == id)
            .expect("all supported hosts are detected");
        if !view.installed {
            return Err(format!(
                "{} was not detected on this device",
                host_profile(id).label
            ));
        }
        let path = knowledge
            .config_path(home)
            .expect("generic MCP hosts declare a config path");
        let entry = entry_for_host(&base_entry, id)?;
        let format = knowledge
            .config_format
            .expect("generic MCP hosts declare a format");
        let action = write_config_for(format, &path, entry)?;
        if let Some(skill_dir) = knowledge.skill_dir(home) {
            write_ramble_skill(&skill_dir)?;
        }
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

fn is_configured(format: ConfigFormat, path: &Path) -> bool {
    match format {
        ConfigFormat::McpServersJson
        | ConfigFormat::GeminiSettingsJson
        | ConfigFormat::AntigravityMcpJson => json_has_server(path, "mcpServers"),
        ConfigFormat::CodexMcpToml => toml_mcp_servers_is_configured(path, true),
        ConfigFormat::GrokMcpToml => toml_mcp_servers_is_configured(path, false),
        ConfigFormat::OpenCodeMcpJson => json_has_server(path, "mcp"),
        ConfigFormat::ReasonixPluginsToml => reasonix_is_configured(path),
    }
}

fn write_config_for(
    format: ConfigFormat,
    path: &Path,
    entry: Value,
) -> Result<&'static str, String> {
    match format {
        ConfigFormat::McpServersJson => write_json_config(path, entry),
        // Gemini CLI expects `httpUrl` instead of `url` and rejects `type`.
        ConfigFormat::GeminiSettingsJson => write_json_config(path, gemini_entry(entry)),
        // Antigravity IDE expects `serverUrl` instead of `url` and rejects `type`.
        ConfigFormat::AntigravityMcpJson => write_json_config(path, antigravity_entry(entry)),
        ConfigFormat::CodexMcpToml => write_codex_config(path, &entry),
        ConfigFormat::GrokMcpToml => write_grok_config(path, &entry),
        ConfigFormat::OpenCodeMcpJson => write_opencode_config(path, &entry),
        ConfigFormat::ReasonixPluginsToml => write_reasonix_config(path, &entry),
    }
}

fn gemini_entry(mut entry: Value) -> Value {
    if let Some(object) = entry.as_object_mut() {
        object.remove("type");
        if let Some(url) = object.remove("url") {
            object.insert("httpUrl".to_owned(), url);
        }
    }
    entry
}

fn antigravity_entry(mut entry: Value) -> Value {
    if let Some(object) = entry.as_object_mut() {
        object.remove("type");
        if let Some(url) = object.remove("url").or_else(|| object.remove("httpUrl")) {
            object.insert("serverUrl".to_owned(), url);
        }
    }
    entry
}

fn json_has_server(path: &Path, section: &str) -> bool {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .and_then(|value| value.get(section)?.get(SERVER_ID).cloned())
        .is_some()
}

fn toml_mcp_servers_is_configured(path: &Path, reject_env: bool) -> bool {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| content.parse::<DocumentMut>().ok())
        .and_then(|document| {
            document
                .get("mcp_servers")?
                .get(SERVER_ID)
                .and_then(Item::as_table)
                .filter(|server| !reject_env || !server.contains_key("env"))
                .map(|_| ())
        })
        .is_some()
}

#[cfg(test)]
fn codex_is_configured(path: &Path) -> bool {
    toml_mcp_servers_is_configured(path, true)
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
    write_toml_mcp_servers_config(path, entry, "http_headers")
}

fn write_grok_config(path: &Path, entry: &Value) -> Result<&'static str, String> {
    write_toml_mcp_servers_config(path, entry, "headers")
}

fn write_toml_mcp_servers_config(
    path: &Path,
    entry: &Value,
    headers_key: &str,
) -> Result<&'static str, String> {
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
    if document
        .get("mcp_servers")
        .and_then(Item::as_table)
        .is_none()
    {
        let mut table = Table::new();
        table.set_implicit(true);
        document["mcp_servers"] = Item::Table(table);
    }
    let mut server = Table::new();
    server["url"] = value(url);
    let mut headers = Table::new();
    headers["Authorization"] = value(authorization);
    if let Some(host_id) = host_id {
        headers[HOST_HEADER] = value(host_id);
    }
    server[headers_key] = Item::Table(headers);
    let before = document.to_string();
    document["mcp_servers"][SERVER_ID] = Item::Table(server);
    let after = document.to_string();
    if before == after {
        return Ok("unchanged");
    }
    write_config(path, after.as_bytes())?;
    Ok(if existed { "updated" } else { "created" })
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

fn write_config(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    fs::write(path, contents)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))
}

/// Write the bundled `ramble` skill into `<skill_dir>/ramble/SKILL.md`.
/// Idempotent: byte-equal content leaves the file untouched.
fn write_ramble_skill(skill_dir: &Path) -> Result<&'static str, String> {
    let target = skill_dir.join("ramble").join("SKILL.md");
    let existed = target.exists();
    if existed {
        let current = fs::read_to_string(&target)
            .map_err(|error| format!("Could not read {}: {error}", target.display()))?;
        if current == RAMBLE_SKILL_MD {
            return Ok("unchanged");
        }
    }
    let parent = target
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", target.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    fs::write(&target, RAMBLE_SKILL_MD)
        .map_err(|error| format!("Could not write {}: {error}", target.display()))?;
    Ok(if existed { "updated" } else { "created" })
}

#[cfg(test)]
#[path = "install/tests.rs"]
mod tests;
