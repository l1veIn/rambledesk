//! Gemini CLI host: `~/.gemini/settings.json` with the shared `mcpServers`
//! JSON shape, but `httpUrl` instead of `url` and no `type` field.

use serde_json::Value;
use std::path::{Path, PathBuf};

use super::{McpHost, write_json_config};

pub(super) struct GeminiHost;

pub(super) const HOST: &'static dyn McpHost = &GeminiHost;

impl McpHost for GeminiHost {
    fn id(&self) -> &'static str {
        "gemini"
    }

    fn executable(&self) -> Option<&'static str> {
        Some("gemini")
    }

    fn config_path(&self, home: &Path) -> PathBuf {
        home.join(".gemini").join("settings.json")
    }

    fn marker_path(&self, home: &Path) -> PathBuf {
        home.join(".gemini")
    }

    fn write_config(&self, path: &Path, mut entry: Value) -> Result<&'static str, String> {
        // Gemini CLI expects `httpUrl` instead of `url` and rejects `type`.
        if let Some(object) = entry.as_object_mut() {
            object.remove("type");
            if let Some(url) = object.remove("url") {
                object.insert("httpUrl".to_owned(), url);
            }
        }
        write_json_config(path, entry)
    }
}
