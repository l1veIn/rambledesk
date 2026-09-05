// Adapted from Codeg 3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1
// src-tauri/src/acp/registry.rs (Apache-2.0). Changed: RambleDesk verification evidence,
// official DSH entry, explicit bridge dependencies, manual bootstrap/binary paths.
use rambledesk_core::*;

pub fn catalog() -> Vec<AgentCatalogEntry> {
    macro_rules! npm {
        ($id:expr,$name:expr,$host:expr,$package:expr,$version:expr,$command:expr,$node:expr,$bridge:expr,$args:expr) => {
            AgentCatalogEntry {
                id: $id.into(), name: $name.into(), host_id: $host.into(),
                description: "ACP launch recipe from Codeg's pinned catalog; installation does not verify managed feedback support.".into(),
                connection_kind: if $bridge { AgentConnectionKind::Bridge } else { AgentConnectionKind::Native },
                distribution: AgentDistribution::Npm { package: $package.into(), pinned_version: $version.into(), command: $command.into(), node_required: $node.into() },
                args: $args.iter().map(|value: &&str| (*value).into()).collect(), dependencies: vec![],
                verification: AgentVerification { status: AgentVerificationStatus::Unverified, versions: vec![], note: "Installed Agents can be selected directly. Check the connection to diagnose ACP access; model-driven command feedback still requires verification.".into() },
            }
        }
    }
    let mut entries = vec![
        npm!(
            "claude-acp",
            "Claude Code",
            "claude",
            "@agentclientprotocol/claude-agent-acp",
            "0.73.0",
            "claude-agent-acp",
            "22.0.0",
            true,
            []
        ),
        npm!(
            "codex-acp",
            "Codex CLI",
            "codex",
            "@agentclientprotocol/codex-acp",
            "1.8.0",
            "codex-acp",
            "20.0.0",
            true,
            []
        ),
        // Project trust remains an explicit Agent permission; do not copy Codeg's --skip-trust.
        npm!(
            "gemini",
            "Gemini CLI",
            "gemini",
            "@google/gemini-cli",
            "0.57.0",
            "gemini",
            "20.0.0",
            false,
            ["--acp"]
        ),
        npm!(
            "openclaw-acp",
            "OpenClaw",
            "openclaw",
            "openclaw",
            "2026.8.1",
            "openclaw",
            "22.22.3",
            false,
            ["acp"]
        ),
        npm!(
            "cline",
            "Cline",
            "cline",
            "cline",
            "3.0.61",
            "cline",
            "22.0.0",
            false,
            ["--acp"]
        ),
        npm!(
            "codebuddy",
            "CodeBuddy",
            "codebuddy",
            "@tencent-ai/codebuddy-code",
            "2.143.0",
            "codebuddy",
            "22.0.0",
            false,
            ["--acp"]
        ),
        npm!(
            "kimi",
            "Kimi Code",
            "kimi",
            "@moonshot-ai/kimi-code",
            "0.40.1",
            "kimi",
            "22.19.0",
            false,
            ["acp"]
        ),
        npm!(
            "pi-acp",
            "Pi",
            "pi",
            "pi-acp",
            "0.0.33",
            "pi-acp",
            "22.0.0",
            true,
            []
        ),
        npm!(
            "grok",
            "Grok",
            "grok",
            "@xai-official/grok",
            "1.0.13",
            "grok",
            "20.0.0",
            false,
            ["--no-auto-update", "agent", "stdio"]
        ),
        npm!(
            "deepseek-acp",
            "DeepSeek ACP",
            "dsh",
            "deepseek-acp",
            "0.8.0",
            "deepseek-acp",
            "22.0.0",
            true,
            []
        ),
        npm!(
            "dsh",
            "DeepSeek Harness",
            "dsh",
            "@deepseek-ai/dsh",
            "0.1.2-rc.1",
            "dsh",
            "22.0.0",
            false,
            ["--profile", "acp"]
        ),
        npm!(
            "qoder",
            "Qoder",
            "qoder",
            "@qoder-ai/qodercli",
            "1.1.41",
            "qoder",
            "20.0.0",
            false,
            ["--acp"]
        ),
    ];
    for (id, name, version, command, args, url, instructions) in [
        (
            "opencode",
            "OpenCode",
            "1.18.26",
            "opencode",
            vec!["acp"],
            "https://github.com/anomalyco/opencode/releases/tag/v1.18.26",
            "Download the release archive for your OS/architecture, extract its executable and add that directory to PATH.",
        ),
        (
            "cursor",
            "Cursor",
            "2026.08.31-4057e58",
            "cursor-agent",
            vec!["acp"],
            "https://cursor.com/docs/cli/installation",
            "Install the Cursor CLI using the vendor instructions. Keep its complete runtime directory; copying only its entry script is insufficient.",
        ),
        (
            "hermes",
            "Hermes Agent",
            "0.21.0",
            "hermes",
            vec!["acp"],
            "https://github.com/NousResearch/hermes-agent",
            "Follow the official installation instructions for Hermes and its Python runtime, then make hermes available on PATH. The npm bootstrap changes a separate runtime and is not managed by this installer.",
        ),
        (
            "antigravity",
            "Google Antigravity",
            "1.0.0",
            "agy_acp_server",
            if cfg!(target_os = "linux") {
                vec!["--uid="]
            } else {
                vec![]
            },
            "https://github.com/agentclientprotocol/registry",
            "Locate the Antigravity entry in the ACP registry, download the archive matching your OS/architecture, preserve its directory layout, and add agy_acp_server to PATH. The catalog version is not the upstream binary build identifier.",
        ),
    ] {
        entries.push(AgentCatalogEntry {
            id: id.into(), name: name.into(), host_id: id.into(), description: instructions.into(), connection_kind: AgentConnectionKind::Native,
            distribution: AgentDistribution::Manual { command: command.into(), version: version.into(), instructions: instructions.into(), docs_url: url.into() },
            args: args.into_iter().map(String::from).collect(), dependencies: vec![],
            verification: AgentVerification { status: AgentVerificationStatus::Unverified, versions: vec![], note: "Automatic binary/bootstrap installation is not implemented; detect an existing installation and run its connection check.".into() },
        });
    }
    for entry in &mut entries {
        match entry.id.as_str() {
            "deepseek-acp" | "dsh" => entry.verification = AgentVerification {
                status: AgentVerificationStatus::Unverified,
                versions: if entry.id == "dsh" { vec!["0.1.2-rc.1".into()] } else { vec!["0.8.0".into()] },
                note: "Previous Windows testing covered ACP sessions, recovery and deletion isolation. The shared command feedback workflow still requires model-driven verification with this installed version.".into(),
            },
            "pi-acp" => {
                entry.dependencies.push(AgentDependency { command: "pi".into(), required: true, package: Some("@earendil-works/pi-coding-agent".into()), pinned_version: Some("0.83.0".into()), instructions: "The bridge launches pi --mode rpc. Its required Pi CLI is installed beside the bridge in the managed prefix.".into() });
                entry.verification = AgentVerification { status: AgentVerificationStatus::Unverified, versions: vec!["0.0.33".into(), "Pi 0.83.0".into()], note: "Pi runs through its ACP bridge with the native Pi CLI. RambleDesk feedback uses the shared command workflow; model-driven sessions still require connection and authentication verification.".into() };
            },
            "openclaw-acp" => entry.verification = AgentVerification { status: AgentVerificationStatus::Unverified, versions: vec!["2026.8.1".into()], note: "This release does not accept ACP-injected MCP servers. RambleDesk feedback uses a command, so MCP support is not required; the model-driven feedback workflow remains unverified.".into() },
            "claude-acp" | "codex-acp" => entry.dependencies.push(AgentDependency {
                command: if entry.id == "claude-acp" { "claude" } else { "codex" }.into(), required: false, package: None, pinned_version: None,
                instructions: "The vendor CLI and ACP bridge are distinct programs and versions. Vendor installation alone does not install the ACP bridge; existing vendor authentication may be shared.".into(),
            }),
            _ => {}
        }
    }
    entries
}

pub(super) fn entry(id: &str) -> Result<AgentCatalogEntry, AgentDriverError> {
    catalog()
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| AgentDriverError::new("Unknown Agent catalog entry"))
}
