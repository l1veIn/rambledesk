//! OpenCode host: `~/.config/opencode/opencode.json` with a `mcp.<name>`
//! entry shaped as `{ type: "remote", url, enabled, headers }`.

use serde_json::{Map, Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

use super::{McpHost, SERVER_ID, write_config};

pub(super) struct OpenCodeHost;

pub(super) const HOST: &'static dyn McpHost = &OpenCodeHost;

impl McpHost for OpenCodeHost {
    fn id(&self) -> &'static str {
        "opencode"
    }

    fn executable(&self) -> Option<&'static str> {
        Some("opencode")
    }

    fn config_path(&self, home: &Path) -> PathBuf {
        home.join(".config").join("opencode").join("opencode.json")
    }

    fn marker_path(&self, home: &Path) -> PathBuf {
        home.join(".opencode")
    }

    fn is_configured(&self, path: &Path) -> bool {
        opencode_is_configured(path)
    }

    fn write_config(&self, path: &Path, entry: Value) -> Result<&'static str, String> {
        write_opencode_config(path, &entry)
    }
}

fn opencode_is_configured(path: &Path) -> bool {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .and_then(|value| value.get("mcp")?.get(SERVER_ID).cloned())
        .is_some()
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
