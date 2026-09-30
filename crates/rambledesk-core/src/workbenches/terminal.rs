use super::*;
pub(super) fn draft_valid(data: &TerminalData, state: Option<&WorkbenchState>) -> bool {
    state.is_none() || result(data, state).is_some()
}
pub(super) fn result_has_input(_: Option<&WorkbenchResult>) -> bool {
    false
}
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

pub(super) fn definition() -> WorkbenchDefinition {
    let (name, purpose, returns, interaction) = (
        "Terminal / 终端试用",
        "Try a prepared CLI interactively and quote output in feedback. 命令行工具试用与操作反馈。",
        "Bounded terminal output, latest screen, shell session status with feedback notes",
        "terminal",
    );
    let mut definition = WorkbenchDefinition::new(
        WorkbenchSummary {
            kind: "terminal",
            version: 1,
            name,
            purpose,
            returns,
            interaction,
        },
        describe,
    );
    definition.pre_publish = Some(pre_publish);
    definition.strict_state = true;
    definition.notes_only = true;
    definition.validate_saved_draft = false;
    definition
}
fn describe() -> Result<WorkbenchDescription, ApplicationError> {
    let (input_schema, result_schema, data, instructions) = (
        schemars::schema_for!(TerminalData),
        schemars::schema_for!(TerminalResult),
        WorkbenchData::Terminal(TerminalData {
            cwd: "/path/to/prepared/project".into(),
            shell: None,
            commands: vec![],
        }),
        "Prepare the CLI and provide its absolute working directory on the RambleDesk host. Optional shell names one executable, without arguments; omit it for the host default. Put the trial instructions and copyable commands in a Markdown request attachment. The reviewer copies commands into the terminal and controls execution, including free input, interactive menus and Ctrl+C. Legacy data.commands is accepted for saved requests but is not rendered. Stopping or exiting a shell lets the reviewer explicitly start another session in the same pending request; tab changes and reconnection never launch a new shell. Quote selected output into the shared feedback body. The result freezes at most 16 shell sessions with cwd, shell, dimensions, original ANSI output, latest rendered screen, status and exit_code; exit_code is the shell session outcome, never an inferred per-command outcome. Raw typed input is not recorded, and individual command execution is not inferred. Each session keeps at most 262144 output and 65536 screen Unicode scalar values, with a 600000 total capture limit; truncated explicitly marks incomplete evidence. Feedback notes are required to submit a trial; startup output alone is not feedback. Cancellation publishes no result. Published logs are evidence and must never be replayed as commands.",
    );
    Ok(WorkbenchDescription {
        summary: definition().summary,
        example: WorkbenchSpec {
            kind: "terminal".into(),
            version: 1,
            data,
        },
        input_schema,
        result_schema,
        instructions,
    })
}
pub(super) fn validate(data: &TerminalData) -> Result<(), ApplicationError> {
    if !super::terminal::terminal_input_valid(data) {
        return Err(ApplicationError::invalid_argument(
            "terminal requires cwd, optional shell and 1–20 uniquely identified, single-line suggested commands with visible title and command",
        ));
    }
    Ok(())
}

pub(super) fn result(_: &TerminalData, state: Option<&WorkbenchState>) -> Option<WorkbenchResult> {
    let sessions = match state {
        Some(WorkbenchState::Terminal { sessions }) if terminal_sessions_valid(sessions) => {
            sessions.clone()
        }
        None => Vec::new(),
        _ => return None,
    };
    Some(WorkbenchResult::Terminal(TerminalResult { sessions }))
}
pub(super) fn has_input(_: &TerminalData, _: Option<&WorkbenchState>) -> bool {
    false
}
pub(super) fn complete(_: &TerminalData, result: Option<&WorkbenchResult>) -> bool {
    matches!(result,Some(WorkbenchResult::Terminal(result)) if result.sessions.iter().all(|s|s.status != TerminalTrialStatus::Running))
}
pub(super) fn legacy_actions(_: &TerminalData) -> Vec<ActionInput> {
    Vec::new()
}
fn pre_publish(context: &WorkbenchRuntimeContext<'_>) -> Result<(), ApplicationError> {
    if !context.terminal_is_finished() {
        return Err(ApplicationError::invalid_argument(
            "Stop the terminal and wait for its output to finish before submitting feedback.",
        ));
    }
    Ok(())
}
