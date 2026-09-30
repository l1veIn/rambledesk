use super::*;
use std::collections::HashSet;

/// A prepared CLI trial. Trial instructions and commands belong in request materials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TerminalData {
    #[schemars(length(min = 1, max = 8192))]
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[schemars(length(min = 1, max = 8192))]
    pub shell: Option<String>,
    /// Legacy suggestions retained for saved requests; the terminal does not render them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(length(max = 20))]
    pub commands: Vec<TerminalCommand>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TerminalCommand {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,63}$"))]
    pub id: String,
    #[schemars(length(min = 1, max = 200))]
    pub title: String,
    #[schemars(length(min = 1, max = 4000))]
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[schemars(length(max = 2000))]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum TerminalTrialStatus {
    Running,
    Exited,
    Stopped,
}

/// Captured trial evidence, never a request to execute commands. Raw key input is
/// deliberately omitted: password prompts must not turn into a saved key log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TerminalTrialSession {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,63}$"))]
    pub id: String,
    #[schemars(length(min = 1, max = 8192))]
    pub cwd: String,
    #[schemars(length(min = 1, max = 8192))]
    pub shell: String,
    #[schemars(range(min = 20, max = 300))]
    pub cols: u32,
    #[schemars(range(min = 5, max = 100))]
    pub rows: u32,
    pub status: TerminalTrialStatus,
    /// Shell session exit code, not the outcome of an individual CLI command.
    pub exit_code: Option<i32>,
    /// Bounded original terminal output, including ANSI escape sequences.
    #[schemars(length(max = 262144))]
    pub output: String,
    /// Latest rendered terminal screen, for CLI menus and other TUI views.
    #[schemars(length(max = 65536))]
    pub screen: String,
    /// True if any part of this session's evidence exceeded its capture limit.
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TerminalResult {
    #[schemars(length(max = 16))]
    pub sessions: Vec<TerminalTrialSession>,
}

fn bounded(value: &str, max: usize, visible: bool) -> bool {
    !value.contains('\0') && value.chars().count() <= max && (!visible || !value.trim().is_empty())
}

pub(super) fn terminal_input_valid(data: &TerminalData) -> bool {
    let mut ids = HashSet::new();
    bounded(&data.cwd, 8192, true)
        && data
            .shell
            .as_deref()
            .is_none_or(|shell| bounded(shell, 8192, true))
        && data.commands.len() <= 20
        && data.commands.iter().all(|command| {
            super::validation::valid_item_id(&command.id)
                && ids.insert(&command.id)
                && bounded(&command.title, 200, true)
                && bounded(&command.command, 4000, true)
                && !command.command.chars().any(char::is_control)
                && command
                    .description
                    .as_deref()
                    .is_none_or(|text| bounded(text, 2000, false))
        })
}

pub(super) fn terminal_sessions_valid(sessions: &[TerminalTrialSession]) -> bool {
    let mut ids = HashSet::new();
    let mut total = 0;
    sessions.len() <= 16
        && sessions.iter().all(|session| {
            total += session.output.chars().count() + session.screen.chars().count();
            super::validation::valid_item_id(&session.id)
                && ids.insert(&session.id)
                && bounded(&session.cwd, 8192, true)
                && bounded(&session.shell, 8192, true)
                && (20..=300).contains(&session.cols)
                && (5..=100).contains(&session.rows)
                && (session.status != TerminalTrialStatus::Running || session.exit_code.is_none())
                && bounded(&session.output, 262144, false)
                && bounded(&session.screen, 65536, false)
                && total <= 600000
        })
}
