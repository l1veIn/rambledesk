use serde::Serialize;

const CLAUDE_ICON: &str = include_str!("../assets/icons/claude.svg");
const CODEX_ICON: &str = include_str!("../assets/icons/openai.svg");
const CURSOR_ICON: &str = include_str!("../assets/icons/cursor.svg");
const GEMINI_ICON: &str = include_str!("../assets/icons/google-gemini.svg");
const PI_ICON: &str = include_str!("../assets/icons/pi.svg");
const OPENCODE_ICON: &str = include_str!("../assets/icons/opencode.svg");
const INSPECTOR_ICON: &str = include_str!("../assets/icons/model-context-protocol.svg");
const GENERIC_ICON: &str = include_str!("../assets/icons/generic-terminal.svg");

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdapterPresentation {
    pub id: String,
    pub label: String,
    pub icon_svg: String,
}

pub fn adapter_presentation(host_id: &str) -> AdapterPresentation {
    let normalized = host_id.trim().to_ascii_lowercase();
    let (id, label, icon) = match normalized.as_str() {
        "claude" => ("claude", "Claude Code", CLAUDE_ICON),
        "codex" => ("codex", "Codex", CODEX_ICON),
        "cursor" => ("cursor", "Cursor", CURSOR_ICON),
        "gemini" => ("gemini", "Gemini CLI", GEMINI_ICON),
        "pi" => ("pi", "Pi", PI_ICON),
        "opencode" => ("opencode", "OpenCode", OPENCODE_ICON),
        "inspector" => ("inspector", "MCP Inspector", INSPECTOR_ICON),
        "" | "unknown" | "generic" => ("generic", "Coding Agent", GENERIC_ICON),
        other => (other, other, GENERIC_ICON),
    };
    AdapterPresentation {
        id: id.to_owned(),
        label: label.to_owned(),
        icon_svg: icon.to_owned(),
    }
}

pub fn known_adapter_presentations() -> Vec<AdapterPresentation> {
    [
        "claude",
        "codex",
        "cursor",
        "gemini",
        "pi",
        "opencode",
        "inspector",
        "generic",
    ]
    .into_iter()
    .map(adapter_presentation)
    .collect()
}
