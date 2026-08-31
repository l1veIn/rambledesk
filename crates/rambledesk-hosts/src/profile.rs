use serde::Serialize;

use crate::HOSTS;

const CLAUDE_ICON: &str = include_str!("../assets/icons/claude.svg");
const CODEX_ICON: &str = include_str!("../assets/icons/openai.svg");
const CURSOR_ICON: &str = include_str!("../assets/icons/cursor.svg");
const GEMINI_ICON: &str = include_str!("../assets/icons/google-gemini.svg");
const ANTIGRAVITY_ICON: &str = include_str!("../assets/icons/antigravity.svg");
const GROK_ICON: &str = include_str!("../assets/icons/grok.svg");
const PI_ICON: &str = include_str!("../assets/icons/pi.svg");
const DSH_ICON: &str = include_str!("../assets/icons/dsh.svg");
const OPENCLAW_ICON: &str = include_str!("../assets/icons/openclaw.svg");
const CLINE_ICON: &str = include_str!("../assets/icons/cline.svg");
const HERMES_ICON: &str = include_str!("../assets/icons/hermes.svg");
const CODEBUDDY_ICON: &str = include_str!("../assets/icons/codebuddy.svg");
const KIMI_CODE_ICON: &str = include_str!("../assets/icons/kimi-code.svg");
const QODER_ICON: &str = include_str!("../assets/icons/qoder.svg");
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
        "claude_code" => generic_profile("claude_code", "Claude Code", CLAUDE_ICON),
        "codex" => generic_profile("codex", "Codex", CODEX_ICON),
        "cursor" => generic_profile("cursor", "Cursor", CURSOR_ICON),
        "gemini" => generic_profile("gemini", "Gemini CLI", GEMINI_ICON),
        "antigravity" => generic_profile("antigravity", "Antigravity IDE", ANTIGRAVITY_ICON),
        "grok" => generic_profile("grok", "Grok", GROK_ICON),
        "open_claw" => generic_profile("open_claw", "OpenClaw", OPENCLAW_ICON),
        "cline" => generic_profile("cline", "Cline", CLINE_ICON),
        "hermes" => generic_profile("hermes", "Hermes Agent", HERMES_ICON),
        "code_buddy" => generic_profile("code_buddy", "CodeBuddy", CODEBUDDY_ICON),
        "kimi_code" => generic_profile("kimi_code", "Kimi Code", KIMI_CODE_ICON),
        "qoder" => generic_profile("qoder", "Qoder", QODER_ICON),
        "pi" => (
            "pi",
            "Pi",
            PI_ICON,
            HostAdapter::PiNative,
            ContinuationMode::NotRequired,
        ),
        "dsh" => (
            "dsh",
            "DeepSeek Harness",
            DSH_ICON,
            HostAdapter::PiNative,
            ContinuationMode::NotRequired,
        ),
        "opencode" => generic_profile("opencode", "OpenCode", OPENCODE_ICON),
        "open_code" => generic_profile("open_code", "OpenCode", OPENCODE_ICON),
        "deepseek" => generic_profile("deepseek", "DeepSeek Harness", DSH_ICON),
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
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn pi_profile_declares_native_wait_without_continuation() {
        let profile = host_profile("pi");
        assert_eq!(profile.default_adapter, HostAdapter::PiNative);
        assert_eq!(profile.continuation_mode, ContinuationMode::NotRequired);
    }

    #[test]
    fn dsh_profile_declares_native_wait_without_continuation() {
        let profile = host_profile("dsh");
        assert_eq!(profile.id, "dsh");
        assert_eq!(profile.label, "DeepSeek Harness");
        assert_eq!(profile.default_adapter, HostAdapter::PiNative);
        assert_eq!(profile.continuation_mode, ContinuationMode::NotRequired);
        assert!(profile.icon_svg.contains("svg"));
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
    fn antigravity_profile_declares_generic_mcp_with_manual_continuation() {
        let profile = host_profile("antigravity");
        assert_eq!(profile.id, "antigravity");
        assert_eq!(profile.label, "Antigravity IDE");
        assert_eq!(profile.default_adapter, HostAdapter::GenericMcp);
        assert_eq!(profile.continuation_mode, ContinuationMode::Manual);
        assert!(profile.icon_svg.contains("svg"));
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

    #[test]
    fn every_builtin_acp_agent_has_a_named_non_generic_mark() {
        let expected = [
            ("claude_code", "Claude Code"),
            ("codex", "Codex"),
            ("gemini", "Gemini CLI"),
            ("open_claw", "OpenClaw"),
            ("open_code", "OpenCode"),
            ("cline", "Cline"),
            ("hermes", "Hermes Agent"),
            ("code_buddy", "CodeBuddy"),
            ("kimi_code", "Kimi Code"),
            ("pi", "Pi"),
            ("grok", "Grok"),
            ("cursor", "Cursor"),
            ("deepseek", "DeepSeek Harness"),
            ("qoder", "Qoder"),
            ("antigravity", "Antigravity IDE"),
        ];
        let mut marks = HashSet::new();

        for (id, label) in expected {
            let profile = host_profile(id);
            assert_eq!(profile.id, id);
            assert_eq!(profile.label, label);
            assert_ne!(profile.icon_svg, GENERIC_ICON, "{id} used the generic mark");
            assert!(profile.icon_svg.contains("<svg"), "{id} mark is not SVG");
            marks.insert(profile.icon_svg);
        }

        assert_eq!(
            marks.len(),
            expected.len(),
            "ACP Agent marks must be distinct"
        );
    }

    #[test]
    fn legacy_adapter_ids_share_marks_with_their_acp_agent_ids() {
        for (legacy, acp) in [
            ("claude", "claude_code"),
            ("opencode", "open_code"),
            ("dsh", "deepseek"),
        ] {
            assert_eq!(host_profile(legacy).icon_svg, host_profile(acp).icon_svg);
        }
    }
}
