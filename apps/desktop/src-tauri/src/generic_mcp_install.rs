//! Generic MCP adapter host discovery and configuration install.
//!
//! Hosts are registered as [`McpHost`] implementations in per-host submodules;
//! [`ALL_HOSTS`] is the single registration point. Adding a new host means
//! adding one submodule plus one entry in [`ALL_HOSTS`]; shared JSON helpers
//! live in this module.

use serde::Serialize;
use serde_json::{Map, Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

use rambledesk_hosts::{HostProfile, host_profile};

use rambledesk_core::find_executable;

mod claude;
mod codex;
mod cursor;
mod gemini;
mod opencode;
mod reasonix;

const SERVER_ID: &str = "rambledesk";
const HOST_ENV_KEY: &str = rambledesk_local_server::HOST_ENV_KEY;
const HOST_HEADER: &str = rambledesk_local_server::HOST_HEADER;

/// Registered hosts. This is the registration point for new hosts.
pub(super) const ALL_HOSTS: &[&'static dyn McpHost] = &[
    claude::HOST,
    codex::HOST,
    cursor::HOST,
    gemini::HOST,
    opencode::HOST,
    reasonix::HOST,
];

/// A host whose MCP configuration RambleDesk can detect and install.
pub(super) trait McpHost {
    /// Stable host id, also used as the `X-RambleDesk-Host` header value.
    fn id(&self) -> &'static str;

    /// Display profile (label, icon) from the shared host registry.
    fn profile(&self) -> HostProfile {
        host_profile(self.id())
    }

    /// Executable name for PATH-based detection; `None` means the marker
    /// directory is the only detection signal.
    fn executable(&self) -> Option<&'static str> {
        None
    }

    /// Where this host's MCP configuration lives.
    fn config_path(&self, home: &Path) -> PathBuf;

    /// Directory whose existence marks the host as installed.
    fn marker_path(&self, home: &Path) -> PathBuf {
        self.config_path(home)
            .parent()
            .unwrap_or(home)
            .to_path_buf()
    }

    /// Whether the RambleDesk server entry is already configured. Defaults to
    /// the shared `mcpServers` JSON shape.
    fn is_configured(&self, path: &Path) -> bool {
        json_is_configured(path)
    }

    /// Install the server entry. `entry` is already host-stamped by
    /// [`entry_for_host`]; implementations may reshape it for their host.
    fn write_config(&self, path: &Path, entry: Value) -> Result<&'static str, String>;
}

fn from_id(id: &str) -> Option<&'static dyn McpHost> {
    ALL_HOSTS.iter().copied().find(|host| host.id() == id)
}

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
    ALL_HOSTS
        .iter()
        .map(|host| {
            let config_path = host.config_path(home);
            let profile = host.profile();
            McpHostView {
                id: host.id(),
                name: profile.label,
                icon_svg: profile.icon_svg,
                installed: host.marker_path(home).exists()
                    || host.executable().and_then(find_executable).is_some(),
                configured: host.is_configured(&config_path),
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
        let host = from_id(id).ok_or_else(|| format!("Unsupported host: {id}"))?;
        let view = detected
            .iter()
            .find(|candidate| candidate.id == host.id())
            .expect("all supported hosts are detected");
        if !view.installed {
            return Err(format!(
                "{} was not detected on this device",
                host.profile().label
            ));
        }
        let path = host.config_path(home);
        let entry = entry_for_host(&base_entry, host.id())?;
        let action = host.write_config(&path, entry)?;
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

/// Shared detection for hosts using the `mcpServers` JSON shape
/// (Claude Code, Cursor, Gemini CLI).
pub(super) fn json_is_configured(path: &Path) -> bool {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .and_then(|value| value.get("mcpServers")?.get(SERVER_ID).cloned())
        .is_some()
}

/// Shared writer for hosts using the `mcpServers` JSON shape.
pub(super) fn write_json_config(path: &Path, entry: Value) -> Result<&'static str, String> {
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

pub(super) fn write_config(path: &Path, contents: &[u8]) -> Result<(), String> {
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
