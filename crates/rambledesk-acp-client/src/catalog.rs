//! Static metadata for the ACP Agents bundled by RambleDesk.
//!
//! The pins and launch artifacts mirror Codeg commit
//! `769610c626f1fc4b18c11d3e289326acf097b99f`. This Module is deliberately
//! side-effect free: discovery, installation, authentication and capability
//! negotiation belong to higher-level Adapters.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinAgentSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub registry_id: &'static str,
    /// Whether an MCP server supplied in ACP `session/new` reaches the Agent.
    pub supports_session_mcp: bool,
    pub distribution: BuiltinAgentDistribution,
    pub access_modes: BuiltinAccessModes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinAgentDistribution {
    Npm {
        version: &'static str,
        package: &'static str,
        command: &'static str,
        args: &'static [&'static str],
        env: &'static [(&'static str, &'static str)],
        node_minimum: &'static str,
    },
    Binary {
        version: &'static str,
        command: &'static str,
        args: &'static [&'static str],
        env: &'static [(&'static str, &'static str)],
        artifacts: &'static [PlatformArtifact],
        directory_entry: Option<BinaryDirectoryEntry>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformArtifact {
    pub platform: &'static str,
    pub url: &'static str,
    pub sha256: Option<&'static str>,
}

/// Entry point for an archive that must remain an intact directory tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BinaryDirectoryEntry {
    pub unix: &'static str,
    pub windows: &'static str,
    pub required_siblings: PlatformFiles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformFiles {
    pub unix: &'static [&'static str],
    pub windows: &'static [&'static str],
}

impl PlatformFiles {
    pub const NONE: Self = Self {
        unix: &[],
        windows: &[],
    };
}

/// Known values for mapping RambleDesk access modes onto an Agent's ACP
/// config selector. Empty slices mean the pinned Agent has no verified mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinAccessModes {
    pub selector_ids: &'static [&'static str],
    pub selector_categories: &'static [&'static str],
    pub read_only: &'static [&'static str],
    pub workspace_write: &'static [&'static str],
    pub yolo: &'static [&'static str],
}

const NO_ACCESS_MODES: BuiltinAccessModes = BuiltinAccessModes {
    selector_ids: &[],
    selector_categories: &[],
    read_only: &[],
    workspace_write: &[],
    yolo: &[],
};

const MODE_SELECTOR: (&[&str], &[&str]) = (&["mode"], &["mode"]);

const CLAUDE_ACCESS: BuiltinAccessModes = BuiltinAccessModes {
    selector_ids: MODE_SELECTOR.0,
    selector_categories: MODE_SELECTOR.1,
    read_only: &["plan"],
    workspace_write: &["default", "acceptEdits"],
    yolo: &["bypassPermissions"],
};

const CODEX_ACCESS: BuiltinAccessModes = BuiltinAccessModes {
    selector_ids: MODE_SELECTOR.0,
    selector_categories: MODE_SELECTOR.1,
    // codex-acp 1.7.0 has no genuinely read-only preset.
    read_only: &[],
    // The misleadingly named `read-only` preset is workspace-write with every
    // escalation reviewed by the user.
    workspace_write: &["read-only"],
    yolo: &["agent-full-access"],
};

const GEMINI_ACCESS: BuiltinAccessModes = BuiltinAccessModes {
    selector_ids: MODE_SELECTOR.0,
    selector_categories: MODE_SELECTOR.1,
    read_only: &["plan"],
    workspace_write: &["default", "auto_edit"],
    yolo: &["yolo"],
};

const OPENCODE_ACCESS: BuiltinAccessModes = BuiltinAccessModes {
    selector_ids: MODE_SELECTOR.0,
    selector_categories: MODE_SELECTOR.1,
    read_only: &["plan"],
    workspace_write: &["build"],
    yolo: &[],
};

const CLINE_ACCESS: BuiltinAccessModes = BuiltinAccessModes {
    selector_ids: MODE_SELECTOR.0,
    selector_categories: MODE_SELECTOR.1,
    read_only: &["plan"],
    workspace_write: &["act"],
    yolo: &[],
};

const KIMI_ACCESS: BuiltinAccessModes = BuiltinAccessModes {
    selector_ids: MODE_SELECTOR.0,
    selector_categories: MODE_SELECTOR.1,
    read_only: &["plan"],
    workspace_write: &["manual"],
    yolo: &["yolo"],
};

const CURSOR_ACCESS: BuiltinAccessModes = BuiltinAccessModes {
    selector_ids: MODE_SELECTOR.0,
    selector_categories: MODE_SELECTOR.1,
    read_only: &["plan", "ask"],
    workspace_write: &["agent"],
    yolo: &[],
};

const QODER_ACCESS: BuiltinAccessModes = BuiltinAccessModes {
    selector_ids: MODE_SELECTOR.0,
    selector_categories: MODE_SELECTOR.1,
    read_only: &["plan"],
    workspace_write: &["default", "acceptEdits"],
    yolo: &["bypassPermissions"],
};

const ANTIGRAVITY_ACCESS: BuiltinAccessModes = BuiltinAccessModes {
    selector_ids: MODE_SELECTOR.0,
    selector_categories: MODE_SELECTOR.1,
    read_only: &[],
    workspace_write: &["default", "auto_edit"],
    yolo: &["yolo"],
};

const HERMES_ACCESS: BuiltinAccessModes = BuiltinAccessModes {
    selector_ids: MODE_SELECTOR.0,
    selector_categories: MODE_SELECTOR.1,
    read_only: &[],
    workspace_write: &["default", "accept_edits"],
    yolo: &["dont_ask"],
};

const DEEPSEEK_ACCESS: BuiltinAccessModes = BuiltinAccessModes {
    selector_ids: &["sandbox"],
    selector_categories: &[],
    read_only: &["read-only"],
    workspace_write: &["workspace-write"],
    yolo: &["danger-full-access"],
};

const GROK_ACCESS: BuiltinAccessModes = BuiltinAccessModes {
    // Grok exposes no ACP mode selector. These are complete root-level
    // arguments consumed through `AccessModeTransport::ProcessArguments`.
    selector_ids: &[],
    selector_categories: &[],
    read_only: &["--permission-mode", "plan"],
    workspace_write: &["--permission-mode", "default"],
    yolo: &["--permission-mode", "bypassPermissions"],
};

const OPENCODE_ARTIFACTS: &[PlatformArtifact] = &[
    PlatformArtifact {
        platform: "darwin-aarch64",
        url: "https://github.com/anomalyco/opencode/releases/download/v1.18.25/opencode-darwin-arm64.zip",
        sha256: None,
    },
    PlatformArtifact {
        platform: "darwin-x86_64",
        url: "https://github.com/anomalyco/opencode/releases/download/v1.18.25/opencode-darwin-x64.zip",
        sha256: None,
    },
    PlatformArtifact {
        platform: "linux-aarch64",
        url: "https://github.com/anomalyco/opencode/releases/download/v1.18.25/opencode-linux-arm64.tar.gz",
        sha256: None,
    },
    PlatformArtifact {
        platform: "linux-x86_64",
        url: "https://github.com/anomalyco/opencode/releases/download/v1.18.25/opencode-linux-x64.tar.gz",
        sha256: None,
    },
    PlatformArtifact {
        platform: "windows-aarch64",
        url: "https://github.com/anomalyco/opencode/releases/download/v1.18.25/opencode-windows-arm64.zip",
        sha256: None,
    },
    PlatformArtifact {
        platform: "windows-x86_64",
        url: "https://github.com/anomalyco/opencode/releases/download/v1.18.25/opencode-windows-x64.zip",
        sha256: None,
    },
];

const CURSOR_ARTIFACTS: &[PlatformArtifact] = &[
    PlatformArtifact {
        platform: "darwin-aarch64",
        url: "https://downloads.cursor.com/lab/2026.08.11-e8db854/darwin/arm64/agent-cli-package.tar.gz",
        sha256: None,
    },
    PlatformArtifact {
        platform: "darwin-x86_64",
        url: "https://downloads.cursor.com/lab/2026.08.11-e8db854/darwin/x64/agent-cli-package.tar.gz",
        sha256: None,
    },
    PlatformArtifact {
        platform: "linux-aarch64",
        url: "https://downloads.cursor.com/lab/2026.08.11-e8db854/linux/arm64/agent-cli-package.tar.gz",
        sha256: None,
    },
    PlatformArtifact {
        platform: "linux-x86_64",
        url: "https://downloads.cursor.com/lab/2026.08.11-e8db854/linux/x64/agent-cli-package.tar.gz",
        sha256: None,
    },
    PlatformArtifact {
        platform: "windows-aarch64",
        url: "https://downloads.cursor.com/lab/2026.08.11-e8db854/windows/arm64/agent-cli-package.zip",
        sha256: None,
    },
    PlatformArtifact {
        platform: "windows-x86_64",
        url: "https://downloads.cursor.com/lab/2026.08.11-e8db854/windows/x64/agent-cli-package.zip",
        sha256: None,
    },
];

const ANTIGRAVITY_ARTIFACTS: &[PlatformArtifact] = &[
    PlatformArtifact {
        platform: "darwin-aarch64",
        url: "https://dl.google.com/agy-extensions/releases/macos/agy-acp-server-agy_acp_server_20260818_01_RC01-darwin-arm64.zip",
        sha256: None,
    },
    PlatformArtifact {
        platform: "linux-aarch64",
        url: "https://dl.google.com/agy-extensions/releases/linux/agy-acp-server-agy_acp_server_20260818_01_RC01-linux-arm64.zip",
        sha256: None,
    },
    PlatformArtifact {
        platform: "linux-x86_64",
        url: "https://dl.google.com/agy-extensions/releases/linux/agy-acp-server-agy_acp_server_20260818_01_RC01-linux-x86_64.zip",
        sha256: None,
    },
    PlatformArtifact {
        platform: "windows-aarch64",
        url: "https://dl.google.com/agy-extensions/releases/windows/agy-acp-server-agy_acp_server_20260818_01_RC01-windows-arm64.zip",
        sha256: None,
    },
    PlatformArtifact {
        platform: "windows-x86_64",
        url: "https://dl.google.com/agy-extensions/releases/windows/agy-acp-server-agy_acp_server_20260818_01_RC01-windows-x86_64.zip",
        sha256: None,
    },
];

const ANTIGRAVITY_ARGS: &[&str] = if cfg!(target_os = "linux") {
    &["--uid="]
} else {
    &[]
};

pub static BUILTIN_AGENTS: &[BuiltinAgentSpec] = &[
    BuiltinAgentSpec {
        id: "claude_code",
        label: "Claude Code",
        registry_id: "claude-acp",
        supports_session_mcp: true,
        distribution: BuiltinAgentDistribution::Npm {
            version: "0.69.0",
            package: "@agentclientprotocol/claude-agent-acp@0.69.0",
            command: "claude-agent-acp",
            args: &[],
            env: &[],
            node_minimum: "22.0.0",
        },
        access_modes: CLAUDE_ACCESS,
    },
    BuiltinAgentSpec {
        id: "codex",
        label: "Codex CLI",
        registry_id: "codex-acp",
        supports_session_mcp: true,
        distribution: BuiltinAgentDistribution::Npm {
            version: "1.7.0",
            package: "@agentclientprotocol/codex-acp@1.7.0",
            command: "codex-acp",
            args: &[],
            env: &[],
            node_minimum: "20.0.0",
        },
        access_modes: CODEX_ACCESS,
    },
    BuiltinAgentSpec {
        id: "gemini",
        label: "Gemini CLI",
        registry_id: "gemini",
        supports_session_mcp: true,
        distribution: BuiltinAgentDistribution::Npm {
            version: "0.57.0",
            package: "@google/gemini-cli@0.57.0",
            command: "gemini",
            args: &["--acp", "--skip-trust"],
            env: &[],
            node_minimum: "20.0.0",
        },
        access_modes: GEMINI_ACCESS,
    },
    BuiltinAgentSpec {
        id: "open_claw",
        label: "OpenClaw",
        registry_id: "openclaw-acp",
        supports_session_mcp: false,
        distribution: BuiltinAgentDistribution::Npm {
            version: "2026.7.1",
            package: "openclaw@2026.7.1",
            command: "openclaw",
            args: &["acp"],
            env: &[],
            node_minimum: "22.22.3",
        },
        access_modes: NO_ACCESS_MODES,
    },
    BuiltinAgentSpec {
        id: "open_code",
        label: "OpenCode",
        registry_id: "opencode",
        supports_session_mcp: true,
        distribution: BuiltinAgentDistribution::Binary {
            version: "1.18.25",
            command: "opencode",
            args: &["acp"],
            env: &[],
            artifacts: OPENCODE_ARTIFACTS,
            directory_entry: None,
        },
        access_modes: OPENCODE_ACCESS,
    },
    BuiltinAgentSpec {
        id: "cline",
        label: "Cline",
        registry_id: "cline",
        supports_session_mcp: true,
        distribution: BuiltinAgentDistribution::Npm {
            version: "3.0.60",
            package: "cline@3.0.60",
            command: "cline",
            args: &["--acp"],
            env: &[],
            node_minimum: "22.0.0",
        },
        access_modes: CLINE_ACCESS,
    },
    BuiltinAgentSpec {
        id: "hermes",
        label: "Hermes Agent",
        registry_id: "hermes",
        // The pinned 0.20.6 ACP handshake does not declare client MCP support.
        supports_session_mcp: false,
        distribution: BuiltinAgentDistribution::Npm {
            version: "0.20.6",
            package: "hermes-agent@0.20.6",
            command: "hermes",
            args: &["acp"],
            env: &[],
            node_minimum: "20.0.0",
        },
        access_modes: HERMES_ACCESS,
    },
    BuiltinAgentSpec {
        id: "code_buddy",
        label: "CodeBuddy",
        registry_id: "codebuddy-code",
        supports_session_mcp: true,
        distribution: BuiltinAgentDistribution::Npm {
            version: "2.141.0",
            package: "@tencent-ai/codebuddy-code@2.141.0",
            command: "codebuddy",
            args: &["--acp"],
            env: &[],
            node_minimum: "22.0.0",
        },
        // The fixed Codeg entry confirms Permission Requests but does not pin
        // a stable config-option access vocabulary for this version.
        access_modes: NO_ACCESS_MODES,
    },
    BuiltinAgentSpec {
        id: "kimi_code",
        label: "Kimi Code",
        registry_id: "kimi-code",
        supports_session_mcp: true,
        distribution: BuiltinAgentDistribution::Npm {
            version: "0.39.1",
            package: "@moonshot-ai/kimi-code@0.39.1",
            command: "kimi",
            args: &["acp"],
            env: &[],
            node_minimum: "22.19.0",
        },
        access_modes: KIMI_ACCESS,
    },
    BuiltinAgentSpec {
        id: "pi",
        label: "Pi",
        registry_id: "pi-acp",
        // pi-acp accepts `mcpServers` but does not forward them to Pi.
        supports_session_mcp: false,
        distribution: BuiltinAgentDistribution::Npm {
            version: "0.0.33",
            package: "pi-acp@0.0.33",
            command: "pi-acp",
            args: &[],
            env: &[("PI_ACP_ENABLE_EMBEDDED_CONTEXT", "true")],
            node_minimum: "22.0.0",
        },
        access_modes: NO_ACCESS_MODES,
    },
    BuiltinAgentSpec {
        id: "grok",
        label: "Grok",
        registry_id: "grok-build",
        supports_session_mcp: true,
        distribution: BuiltinAgentDistribution::Npm {
            version: "1.0.5",
            package: "@xai-official/grok@1.0.5",
            command: "grok",
            args: &["--no-auto-update", "agent", "stdio"],
            env: &[],
            node_minimum: "20.0.0",
        },
        // Grok's access mode is a root-level launch flag, not an ACP config selector.
        access_modes: GROK_ACCESS,
    },
    BuiltinAgentSpec {
        id: "cursor",
        label: "Cursor",
        registry_id: "cursor",
        supports_session_mcp: true,
        distribution: BuiltinAgentDistribution::Binary {
            version: "2026.08.11-e8db854",
            command: "cursor-agent",
            args: &["acp"],
            env: &[],
            artifacts: CURSOR_ARTIFACTS,
            directory_entry: Some(BinaryDirectoryEntry {
                unix: "dist-package/cursor-agent",
                windows: "dist-package/cursor-agent.cmd",
                required_siblings: PlatformFiles::NONE,
            }),
        },
        access_modes: CURSOR_ACCESS,
    },
    BuiltinAgentSpec {
        id: "deepseek",
        label: "DeepSeek Harness",
        registry_id: "deepseek-acp",
        supports_session_mcp: true,
        distribution: BuiltinAgentDistribution::Npm {
            version: "0.7.0",
            package: "deepseek-acp@0.7.0",
            command: "deepseek-acp",
            args: &[],
            env: &[],
            node_minimum: "22.0.0",
        },
        access_modes: DEEPSEEK_ACCESS,
    },
    BuiltinAgentSpec {
        id: "qoder",
        label: "Qoder",
        registry_id: "qoder-cli",
        supports_session_mcp: true,
        distribution: BuiltinAgentDistribution::Npm {
            version: "1.1.33",
            package: "@qoder-ai/qodercli@1.1.33",
            command: "qoder",
            args: &["--acp"],
            env: &[],
            node_minimum: "20.0.0",
        },
        access_modes: QODER_ACCESS,
    },
    BuiltinAgentSpec {
        id: "antigravity",
        label: "Google Antigravity",
        registry_id: "antigravity-acp",
        supports_session_mcp: true,
        distribution: BuiltinAgentDistribution::Binary {
            version: "1.0.0",
            command: "agy_acp_server",
            args: ANTIGRAVITY_ARGS,
            env: &[],
            artifacts: ANTIGRAVITY_ARTIFACTS,
            directory_entry: Some(BinaryDirectoryEntry {
                unix: "agy_acp_server.par",
                windows: "agy_acp_server.exe",
                required_siblings: PlatformFiles {
                    unix: &["localharness_external"],
                    windows: &["localharness_external.exe"],
                },
            }),
        },
        access_modes: ANTIGRAVITY_ACCESS,
    },
];

pub fn builtin_agents() -> &'static [BuiltinAgentSpec] {
    BUILTIN_AGENTS
}

pub fn builtin_agent(id: &str) -> Option<&'static BuiltinAgentSpec> {
    BUILTIN_AGENTS.iter().find(|agent| agent.id == id)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn builtin_identities_and_registry_ids_are_unique() {
        assert_eq!(BUILTIN_AGENTS.len(), 15);
        let ids = BUILTIN_AGENTS
            .iter()
            .map(|agent| agent.id)
            .collect::<HashSet<_>>();
        let registry_ids = BUILTIN_AGENTS
            .iter()
            .map(|agent| agent.registry_id)
            .collect::<HashSet<_>>();
        assert_eq!(ids.len(), BUILTIN_AGENTS.len());
        assert_eq!(registry_ids.len(), BUILTIN_AGENTS.len());
    }

    #[test]
    fn session_mcp_truthfully_excludes_agents_that_cannot_receive_the_toolset() {
        assert!(!builtin_agent("open_claw").unwrap().supports_session_mcp);
        assert!(!builtin_agent("pi").unwrap().supports_session_mcp);
        assert!(!builtin_agent("hermes").unwrap().supports_session_mcp);
    }

    #[test]
    fn cursor_and_antigravity_preserve_their_directory_trees() {
        let cursor = builtin_agent("cursor").unwrap();
        let antigravity = builtin_agent("antigravity").unwrap();

        let BuiltinAgentDistribution::Binary {
            directory_entry: Some(cursor_entry),
            ..
        } = cursor.distribution
        else {
            panic!("Cursor must use a directory-tree binary")
        };
        assert_eq!(cursor_entry.unix, "dist-package/cursor-agent");

        let BuiltinAgentDistribution::Binary {
            artifacts,
            directory_entry: Some(antigravity_entry),
            ..
        } = antigravity.distribution
        else {
            panic!("Antigravity must use a directory-tree binary")
        };
        assert_eq!(artifacts.len(), 5);
        assert!(
            !artifacts
                .iter()
                .any(|artifact| artifact.platform == "darwin-x86_64")
        );
        assert_eq!(
            antigravity_entry.required_siblings.unix,
            &["localharness_external"]
        );
        assert_eq!(
            antigravity_entry.required_siblings.windows,
            &["localharness_external.exe"]
        );
    }

    #[test]
    fn codex_pin_matches_the_audited_codeg_catalog() {
        let codex = builtin_agent("codex").unwrap();
        assert_eq!(codex.registry_id, "codex-acp");
        let BuiltinAgentDistribution::Npm {
            version,
            package,
            command,
            node_minimum,
            ..
        } = codex.distribution
        else {
            panic!("Codex must be npm-distributed")
        };
        assert_eq!(version, "1.7.0");
        assert_eq!(package, "@agentclientprotocol/codex-acp@1.7.0");
        assert_eq!(command, "codex-acp");
        assert_eq!(node_minimum, "20.0.0");
    }
}
