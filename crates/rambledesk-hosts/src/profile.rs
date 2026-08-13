use serde::Serialize;

use crate::HOSTS;

const CLAUDE_ICON: &str = include_str!("../assets/icons/claude.svg");
const CODEX_ICON: &str = include_str!("../assets/icons/openai.svg");
const CURSOR_ICON: &str = include_str!("../assets/icons/cursor.svg");
const GEMINI_ICON: &str = include_str!("../assets/icons/google-gemini.svg");
const GROK_ICON: &str = include_str!("../assets/icons/grok.svg");
const PI_ICON: &str = include_str!("../assets/icons/pi.svg");
const REASONIX_ICON: &str = include_str!("../assets/icons/reasonix.svg");
const OPENCODE_ICON: &str = include_str!("../assets/icons/opencode.svg");
const INSPECTOR_ICON: &str = include_str!("../assets/icons/model-context-protocol.svg");
const GENERIC_ICON: &str = include_str!("../assets/icons/generic-terminal.svg");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAdapter {
    GenericMcp,
    PiNative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationMode {
    NotRequired,
    Manual,
    Native,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HostProfile {
    pub id: String,
    pub label: String,
    pub icon_svg: String,
    pub default_adapter: HostAdapter,
    pub continuation_mode: ContinuationMode,
}

pub fn host_profile(host_id: &str) -> HostProfile {
    let normalized = host_id.trim().to_ascii_lowercase();
    let (id, label, icon, default_adapter, continuation_mode) = match normalized.as_str() {
        "claude" => generic_profile("claude", "Claude Code", CLAUDE_ICON),
        "codex" => generic_profile("codex", "Codex", CODEX_ICON),
        "cursor" => generic_profile("cursor", "Cursor", CURSOR_ICON),
        "gemini" => generic_profile("gemini", "Gemini CLI", GEMINI_ICON),
        "grok" => generic_profile("grok", "Grok", GROK_ICON),
        "pi" => (
            "pi",
            "Pi",
            PI_ICON,
            HostAdapter::PiNative,
            ContinuationMode::NotRequired,
        ),
        "opencode" => generic_profile("opencode", "OpenCode", OPENCODE_ICON),
        "reasonix" => generic_profile("reasonix", "Reasonix", REASONIX_ICON),
        "inspector" => generic_profile("inspector", "MCP Inspector", INSPECTOR_ICON),
        "" | "unknown" | "generic" => generic_profile("generic", "Generic Host", GENERIC_ICON),
        other => generic_profile(other, other, GENERIC_ICON),
    };
    HostProfile {
        id: id.to_owned(),
        label: label.to_owned(),
        icon_svg: icon.to_owned(),
        default_adapter,
        continuation_mode,
    }
}

fn generic_profile<'a>(
    id: &'a str,
    label: &'a str,
    icon: &'static str,
) -> (
    &'a str,
    &'a str,
    &'static str,
    HostAdapter,
    ContinuationMode,
) {
    (
        id,
        label,
        icon,
        HostAdapter::GenericMcp,
        ContinuationMode::Manual,
    )
}

pub fn known_host_profiles() -> Vec<HostProfile> {
    HOSTS
        .iter()
        .map(|knowledge| host_profile(knowledge.id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pi_profile_declares_native_wait_without_continuation() {
        let profile = host_profile("pi");
        assert_eq!(profile.default_adapter, HostAdapter::PiNative);
        assert_eq!(profile.continuation_mode, ContinuationMode::NotRequired);
    }

    #[test]
    fn generic_hosts_declare_manual_continuation() {
        let profile = host_profile("codex");
        assert_eq!(profile.default_adapter, HostAdapter::GenericMcp);
        assert_eq!(profile.continuation_mode, ContinuationMode::Manual);
    }

    #[test]
    fn reasonix_profile_declares_generic_mcp_with_manual_continuation() {
        let profile = host_profile("reasonix");
        assert_eq!(profile.id, "reasonix");
        assert_eq!(profile.label, "Reasonix");
        assert_eq!(profile.default_adapter, HostAdapter::GenericMcp);
        assert_eq!(profile.continuation_mode, ContinuationMode::Manual);
    }

    #[test]
    fn grok_profile_declares_generic_mcp_with_manual_continuation() {
        let profile = host_profile("grok");
        assert_eq!(profile.id, "grok");
        assert_eq!(profile.label, "Grok");
        assert_eq!(profile.default_adapter, HostAdapter::GenericMcp);
        assert_eq!(profile.continuation_mode, ContinuationMode::Manual);
        assert!(profile.icon_svg.contains("Grok"));
    }
}
