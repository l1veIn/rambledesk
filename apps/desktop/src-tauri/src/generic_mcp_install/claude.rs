//! Claude Code host: `~/.claude.json` with the shared `mcpServers` JSON shape.

use serde_json::Value;
use std::path::{Path, PathBuf};

use super::{McpHost, write_json_config};

pub(super) struct ClaudeHost;

pub(super) const HOST: &'static dyn McpHost = &ClaudeHost;

impl McpHost for ClaudeHost {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn executable(&self) -> Option<&'static str> {
        Some("claude")
    }

    fn config_path(&self, home: &Path) -> PathBuf {
        home.join(".claude.json")
    }

    fn marker_path(&self, home: &Path) -> PathBuf {
        home.join(".claude")
    }

    fn write_config(&self, path: &Path, entry: Value) -> Result<&'static str, String> {
        write_json_config(path, entry)
    }
}
