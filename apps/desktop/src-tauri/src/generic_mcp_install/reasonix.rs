//! Reasonix (Go, v1.8+) MCP adapter install support.
//!
//! Reasonix global config lives at `<Reasonix home>/config.toml` (`REASONIX_HOME`
//! override; Windows `%APPDATA%\reasonix`, macOS/Linux `~/.reasonix`). MCP
//! servers are declared as `[[plugins]]` entries; remote HTTP servers use
//! `type = "http"` + `url` + `headers`.

use serde_json::Value;
use std::{env, fs, path::Path, path::PathBuf};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

use super::{McpHost, SERVER_ID, write_config};

pub(super) struct ReasonixHost;

pub(super) const HOST: &'static dyn McpHost = &ReasonixHost;

impl McpHost for ReasonixHost {
    fn id(&self) -> &'static str {
        "reasonix"
    }

    fn executable(&self) -> Option<&'static str> {
        Some("reasonix")
    }

    fn config_path(&self, home: &Path) -> PathBuf {
        reasonix_home(home).join("config.toml")
    }

    fn marker_path(&self, home: &Path) -> PathBuf {
        reasonix_home(home)
    }

    fn is_configured(&self, path: &Path) -> bool {
        reasonix_is_configured(path)
    }

    fn write_config(&self, path: &Path, entry: Value) -> Result<&'static str, String> {
        write_reasonix_config(path, &entry)
    }
}

/// Reasonix home directory. `REASONIX_HOME` overrides for portable installs;
/// otherwise Windows uses `%APPDATA%\reasonix` and macOS/Linux use `~/.reasonix`.
pub(super) fn reasonix_home(home: &Path) -> PathBuf {
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

pub(super) fn reasonix_is_configured(path: &Path) -> bool {
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

pub(super) fn write_reasonix_config(path: &Path, entry: &Value) -> Result<&'static str, String> {
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
