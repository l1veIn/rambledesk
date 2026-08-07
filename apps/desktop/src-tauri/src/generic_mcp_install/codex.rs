//! Codex CLI host: `config.toml` with `[mcp_servers.rambledesk]` plus
//! `http_headers`. `CODEX_HOME` overrides the `.codex` directory.

use serde_json::Value;
use std::{
    env, fs,
    path::{Path, PathBuf},
};
use toml_edit::{DocumentMut, Item, Table, value};

use super::{HOST_HEADER, McpHost, SERVER_ID, write_config};

pub(super) struct CodexHost;

pub(super) const HOST: &'static dyn McpHost = &CodexHost;

impl McpHost for CodexHost {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn executable(&self) -> Option<&'static str> {
        Some("codex")
    }

    fn config_path(&self, home: &Path) -> PathBuf {
        env::var_os("CODEX_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".codex"))
            .join("config.toml")
    }

    fn marker_path(&self, home: &Path) -> PathBuf {
        self.config_path(home)
            .parent()
            .unwrap_or(home)
            .to_path_buf()
    }

    fn is_configured(&self, path: &Path) -> bool {
        codex_is_configured(path)
    }

    fn write_config(&self, path: &Path, entry: Value) -> Result<&'static str, String> {
        write_codex_config(path, &entry)
    }
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
