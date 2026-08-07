//! Cursor host: `~/.cursor/mcp.json` with the shared `mcpServers` JSON shape.

use serde_json::Value;
use std::path::{Path, PathBuf};

use super::{McpHost, write_json_config};

pub(super) struct CursorHost;

pub(super) const HOST: &'static dyn McpHost = &CursorHost;

impl McpHost for CursorHost {
    fn id(&self) -> &'static str {
        "cursor"
    }

    fn executable(&self) -> Option<&'static str> {
        Some("cursor")
    }

    fn config_path(&self, home: &Path) -> PathBuf {
        home.join(".cursor").join("mcp.json")
    }

    fn marker_path(&self, home: &Path) -> PathBuf {
        home.join(".cursor")
    }

    fn write_config(&self, path: &Path, entry: Value) -> Result<&'static str, String> {
        write_json_config(path, entry)
    }
}
