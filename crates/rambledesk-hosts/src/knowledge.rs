//! Host knowledge registry — the single source of truth for what a host is.
//!
//! Every known host declares its identity, detection knowledge (executable
//! name, marker directory) and, when it is installable through the Generic MCP
//! Adapter scheme, its configuration knowledge (config file location and
//! format). Adding a host means adding one entry here; `known_host_profiles`
//! and the installer registry both derive from this list.

use std::{
    env,
    path::{Path, PathBuf},
};

/// Configuration file shapes understood by the Generic MCP Adapter installer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    /// `{ "mcpServers": { "<name>": entry } }` (Claude Code, Cursor).
    McpServersJson,
    /// `mcpServers` JSON but `httpUrl` instead of `url` and no `type` (Gemini CLI).
    GeminiSettingsJson,
    /// `mcpServers` JSON but `serverUrl` instead of `url` and no `type` (Antigravity IDE).
    AntigravityMcpJson,
    /// `[mcp_servers.<name>]` TOML with `http_headers` (Codex CLI).
    CodexMcpToml,
    /// `{ "mcp": { "<name>": remote entry } }` (OpenCode).
    OpenCodeMcpJson,
    /// `[[plugins]]` TOML with `type = "http"` (Reasonix).
    ReasonixPluginsToml,
    /// `[mcp_servers.<name>]` TOML with `url` + `headers` (Grok CLI).
    GrokMcpToml,
}

/// Declarative knowledge about one host.
pub struct HostKnowledge {
    pub id: &'static str,
    /// Executable name for PATH-based detection.
    pub executable: Option<&'static str>,
    /// Configuration/install knowledge; `None` means the host is not
    /// installable through the Generic MCP Adapter scheme.
    pub config_format: Option<ConfigFormat>,
    /// Home-relative directory where the `ramble` skill is installed
    /// (`<skill_dir>/ramble/SKILL.md`), following the Agent Skills convention.
    /// `None` means the host does not receive a skill via the Generic MCP
    /// Adapter.
    skill_dir: Option<&'static str>,
    config_path: Option<fn(&Path) -> PathBuf>,
    marker_path: Option<fn(&Path) -> PathBuf>,
}

impl HostKnowledge {
    pub fn config_path(&self, home: &Path) -> Option<PathBuf> {
        self.config_path.map(|path| path(home))
    }

    pub fn marker_path(&self, home: &Path) -> Option<PathBuf> {
        self.marker_path.map(|path| path(home))
    }

    /// Absolute path of the host's skill directory for the given home, or
    /// `None` when the host does not receive a skill.
    pub fn skill_dir(&self, home: &Path) -> Option<PathBuf> {
        self.skill_dir.map(|relative| home.join(relative))
    }
}

fn home_path(home: &Path, relative: &str) -> PathBuf {
    home.join(relative)
}

fn codex_home(home: &Path) -> PathBuf {
    env::var_os("CODEX_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".codex"))
}

fn codex_config_path(home: &Path) -> PathBuf {
    codex_home(home).join("config.toml")
}

fn codex_marker_path(home: &Path) -> PathBuf {
    codex_config_path(home)
        .parent()
        .unwrap_or(home)
        .to_path_buf()
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

fn reasonix_config_path(home: &Path) -> PathBuf {
    reasonix_home(home).join("config.toml")
}

/// Grok home directory. `GROK_HOME` overrides the default `~/.grok`.
fn grok_home(home: &Path) -> PathBuf {
    env::var_os("GROK_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".grok"))
}

fn grok_config_path(home: &Path) -> PathBuf {
    grok_home(home).join("config.toml")
}

fn antigravity_unified_dir(home: &Path) -> PathBuf {
    home.join(".gemini").join("config")
}

fn antigravity_legacy_dir(home: &Path) -> PathBuf {
    home.join(".gemini").join("antigravity")
}

/// Prefer the post-migration unified MCP file when Antigravity has moved to
/// `~/.gemini/config`; otherwise keep writing the legacy
/// `~/.gemini/antigravity/mcp_config.json`. New installs default to unified.
fn antigravity_config_path(home: &Path) -> PathBuf {
    let unified_dir = antigravity_unified_dir(home);
    let unified_file = unified_dir.join("mcp_config.json");
    let legacy_dir = antigravity_legacy_dir(home);
    let legacy_file = legacy_dir.join("mcp_config.json");
    if unified_dir.join(".migrated").exists() || unified_file.exists() {
        return unified_file;
    }
    if legacy_file.exists() || legacy_dir.exists() {
        return legacy_file;
    }
    unified_file
}

fn antigravity_marker_path(home: &Path) -> PathBuf {
    let unified = antigravity_unified_dir(home);
    if unified.exists()
        || unified.join("mcp_config.json").exists()
        || unified.join(".migrated").exists()
    {
        return unified;
    }
    let legacy = antigravity_legacy_dir(home);
    if legacy.exists() || legacy.join("mcp_config.json").exists() {
        return legacy;
    }
    unified
}

/// Every known host, in UI display order. This is the single registration
/// point: adding a host means adding one entry here.
pub const HOSTS: &[HostKnowledge] = &[
    HostKnowledge {
        id: "claude",
        executable: Some("claude"),
        config_format: Some(ConfigFormat::McpServersJson),
        config_path: Some(|home| home_path(home, ".claude.json")),
        marker_path: Some(|home| home_path(home, ".claude")),
        skill_dir: Some(".claude/skills"),
    },
    HostKnowledge {
        id: "codex",
        executable: Some("codex"),
        config_format: Some(ConfigFormat::CodexMcpToml),
        config_path: Some(codex_config_path),
        marker_path: Some(codex_marker_path),
        skill_dir: Some(".codex/skills"),
    },
    HostKnowledge {
        id: "cursor",
        executable: Some("cursor"),
        config_format: Some(ConfigFormat::McpServersJson),
        config_path: Some(|home| home_path(home, ".cursor/mcp.json")),
        marker_path: Some(|home| home_path(home, ".cursor")),
        skill_dir: Some(".cursor/skills"),
    },
    HostKnowledge {
        id: "gemini",
        executable: Some("gemini"),
        config_format: Some(ConfigFormat::GeminiSettingsJson),
        config_path: Some(|home| home_path(home, ".gemini/settings.json")),
        marker_path: Some(|home| home_path(home, ".gemini")),
        skill_dir: Some(".gemini/skills"),
    },
    HostKnowledge {
        id: "antigravity",
        executable: Some("antigravity"),
        config_format: Some(ConfigFormat::AntigravityMcpJson),
        config_path: Some(antigravity_config_path),
        marker_path: Some(antigravity_marker_path),
        skill_dir: Some(".gemini/antigravity/skills"),
    },
    HostKnowledge {
        id: "grok",
        executable: Some("grok"),
        config_format: Some(ConfigFormat::GrokMcpToml),
        config_path: Some(grok_config_path),
        marker_path: Some(grok_home),
        skill_dir: Some(".grok/skills"),
    },
    HostKnowledge {
        id: "pi",
        executable: Some("pi"),
        config_format: None,
        config_path: None,
        marker_path: None,
        skill_dir: None,
    },
    HostKnowledge {
        id: "dsh",
        executable: Some("dsh"),
        config_format: None,
        config_path: None,
        marker_path: Some(|home| home_path(home, ".dsh")),
        skill_dir: Some(".agents/skills"),
    },
    HostKnowledge {
        id: "opencode",
        executable: Some("opencode"),
        config_format: Some(ConfigFormat::OpenCodeMcpJson),
        config_path: Some(|home| home_path(home, ".config/opencode/opencode.json")),
        marker_path: Some(|home| home_path(home, ".opencode")),
        skill_dir: Some(".config/opencode/skills"),
    },
    HostKnowledge {
        id: "reasonix",
        executable: Some("reasonix"),
        config_format: Some(ConfigFormat::ReasonixPluginsToml),
        config_path: Some(reasonix_config_path),
        marker_path: Some(reasonix_home),
        skill_dir: Some(".agents/skills"),
    },
    HostKnowledge {
        id: "inspector",
        executable: None,
        config_format: None,
        config_path: None,
        marker_path: None,
        skill_dir: None,
    },
    HostKnowledge {
        id: "generic",
        executable: None,
        config_format: None,
        config_path: None,
        marker_path: None,
        skill_dir: None,
    },
];

/// Hosts the Generic MCP Adapter scheme can detect and install.
pub fn generic_mcp_hosts() -> impl Iterator<Item = &'static HostKnowledge> {
    HOSTS.iter().filter(|host| host.config_format.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_ids_are_unique_and_match_profiles() {
        let mut ids: Vec<&str> = HOSTS.iter().map(|host| host.id).collect();
        let unique = {
            let mut sorted = ids.clone();
            sorted.sort_unstable();
            sorted.dedup();
            sorted.len() == ids.len()
        };
        assert!(unique, "host ids must be unique: {ids:?}");
        for id in &ids {
            assert_eq!(crate::host_profile(id).id, *id);
        }
        ids.sort_unstable();
        assert_eq!(
            ids,
            [
                "antigravity",
                "claude",
                "codex",
                "cursor",
                "dsh",
                "gemini",
                "generic",
                "grok",
                "inspector",
                "opencode",
                "pi",
                "reasonix"
            ]
        );
    }

    #[test]
    fn generic_mcp_hosts_are_exactly_the_installable_hosts() {
        let ids: Vec<&str> = generic_mcp_hosts().map(|host| host.id).collect();
        assert_eq!(
            ids,
            [
                "claude",
                "codex",
                "cursor",
                "gemini",
                "antigravity",
                "grok",
                "opencode",
                "reasonix"
            ]
        );
    }

    #[test]
    fn antigravity_prefers_unified_config_and_falls_back_to_legacy() {
        let directory = tempfile::tempdir().expect("temp dir");
        let home = directory.path();
        let host = HOSTS
            .iter()
            .find(|host| host.id == "antigravity")
            .expect("host");
        assert_eq!(
            host.config_path(home).expect("default config"),
            home.join(".gemini").join("config").join("mcp_config.json")
        );

        std::fs::create_dir_all(home.join(".gemini").join("antigravity")).expect("legacy dir");
        assert_eq!(
            host.config_path(home).expect("legacy config"),
            home.join(".gemini")
                .join("antigravity")
                .join("mcp_config.json")
        );
        assert_eq!(
            host.marker_path(home).expect("legacy marker"),
            home.join(".gemini").join("antigravity")
        );

        std::fs::create_dir_all(home.join(".gemini").join("config")).expect("unified dir");
        std::fs::write(home.join(".gemini").join("config").join(".migrated"), b"").expect("marker");
        assert_eq!(
            host.config_path(home).expect("unified config"),
            home.join(".gemini").join("config").join("mcp_config.json")
        );
        assert_eq!(
            host.marker_path(home).expect("unified marker"),
            home.join(".gemini").join("config")
        );
    }

    #[test]
    fn reasonix_paths_follow_reasonix_home() {
        let directory = tempfile::tempdir().expect("temp dir");
        let home = directory.path();
        let host = HOSTS
            .iter()
            .find(|host| host.id == "reasonix")
            .expect("host");
        #[cfg(windows)]
        let previous_appdata = std::env::var_os("APPDATA");
        #[cfg(windows)]
        unsafe {
            std::env::remove_var("APPDATA");
        }
        let marker = host.marker_path(home).expect("marker");
        let config = host.config_path(home).expect("config");
        #[cfg(windows)]
        match previous_appdata {
            Some(value) => unsafe { std::env::set_var("APPDATA", value) },
            None => unsafe { std::env::remove_var("APPDATA") },
        }
        #[cfg(windows)]
        let expected_home = home.join("AppData").join("Roaming").join("reasonix");
        #[cfg(not(windows))]
        let expected_home = home.join(".reasonix");
        assert_eq!(marker, expected_home);
        assert_eq!(config, expected_home.join("config.toml"));
    }

    #[test]
    fn codex_paths_follow_cod_ex_home_override() {
        let directory = tempfile::tempdir().expect("temp dir");
        let home = directory.path();
        let host = HOSTS.iter().find(|host| host.id == "codex").expect("host");
        let previous = std::env::var_os("CODEX_HOME");
        unsafe {
            std::env::set_var("CODEX_HOME", home.join("portable-codex"));
        }
        let config = host.config_path(home).expect("config");
        let marker = host.marker_path(home).expect("marker");
        match previous {
            Some(value) => unsafe { std::env::set_var("CODEX_HOME", value) },
            None => unsafe { std::env::remove_var("CODEX_HOME") },
        }
        assert_eq!(config, home.join("portable-codex").join("config.toml"));
        assert_eq!(marker, home.join("portable-codex"));
    }

    #[test]
    fn grok_paths_follow_grok_home_override() {
        let directory = tempfile::tempdir().expect("temp dir");
        let home = directory.path();
        let host = HOSTS.iter().find(|host| host.id == "grok").expect("host");
        let previous = std::env::var_os("GROK_HOME");
        unsafe {
            std::env::set_var("GROK_HOME", home.join("portable-grok"));
        }
        let config = host.config_path(home).expect("config");
        let marker = host.marker_path(home).expect("marker");
        match previous {
            Some(value) => unsafe { std::env::set_var("GROK_HOME", value) },
            None => unsafe { std::env::remove_var("GROK_HOME") },
        }
        assert_eq!(config, home.join("portable-grok").join("config.toml"));
        assert_eq!(marker, home.join("portable-grok"));
    }
}
