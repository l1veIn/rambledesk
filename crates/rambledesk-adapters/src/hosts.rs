use std::{
    env, fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use crate::{GenericWakeupAdapter, WakePayload, WakeResult, WakeupAdapter};

const ACCEPTANCE_TIMEOUT: Duration = Duration::from_millis(900);

pub fn known_host_wakeup_adapters() -> Vec<Arc<dyn WakeupAdapter>> {
    [
        HostKind::ClaudeCode,
        HostKind::Codex,
        HostKind::Pi,
        HostKind::OpenCode,
    ]
    .into_iter()
    .map(|kind| Arc::new(HostWakeupAdapter::new(kind)) as Arc<dyn WakeupAdapter>)
    .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostKind {
    ClaudeCode,
    Codex,
    Pi,
    OpenCode,
}

impl HostKind {
    fn id(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude",
            Self::Codex => "codex",
            Self::Pi => "pi",
            Self::OpenCode => "opencode",
        }
    }

    fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::ClaudeCode => &["claude", "claudecode", "claudecodecli"],
            Self::Codex => &["codex", "openaicodex"],
            Self::Pi => &["pi"],
            Self::OpenCode => &["opencode", "open_code"],
        }
    }

    fn command_specs(self, payload: &WakePayload) -> Vec<CommandSpec> {
        let prompt = payload.resume_prompt();
        let session_id = payload.session_id.trim().to_owned();
        let current_dir = project_current_dir(payload);
        match self {
            Self::ClaudeCode => {
                let home = env::var_os("HOME").filter(|value| !value.is_empty());
                claude_command_specs(payload, home.as_ref().map(Path::new))
            }
            Self::Codex => binary_candidates(
                "RAMBLEDESK_CODEX_BIN",
                [
                    "/Applications/ChatGPT.app/Contents/Resources/codex",
                    "codex",
                ]
                .as_slice(),
            )
            .into_iter()
            .map(|program| CommandSpec {
                program,
                args: vec![
                    "exec".to_owned(),
                    "resume".to_owned(),
                    session_id.clone(),
                    prompt.clone(),
                    "--json".to_owned(),
                ],
                current_dir: current_dir.clone(),
            })
            .collect(),
            Self::Pi => binary_candidates("RAMBLEDESK_PI_BIN", ["pi"].as_slice())
                .into_iter()
                .map(|program| CommandSpec {
                    program,
                    args: pi_args(&session_id, &prompt),
                    current_dir: current_dir.clone(),
                })
                .collect(),
            Self::OpenCode => {
                let mut specs = Vec::new();
                for program in binary_candidates(
                    "RAMBLEDESK_OPENCODE_BIN",
                    [opencode_home_binary(), Some(PathBuf::from("opencode"))]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .as_slice(),
                ) {
                    let mut args = vec!["run".to_owned()];
                    if let Some(server_url) = opencode_server_url() {
                        args.push("--attach".to_owned());
                        args.push(server_url);
                    }
                    if let Some(project_root) = payload.project_root_path.as_deref().and_then(clean)
                    {
                        args.push("--dir".to_owned());
                        args.push(project_root.to_owned());
                    }
                    args.extend(["--session".to_owned(), session_id.clone(), prompt.clone()]);
                    specs.push(CommandSpec {
                        program,
                        args,
                        current_dir: current_dir.clone(),
                    });
                }
                specs
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandSpec {
    program: PathBuf,
    args: Vec<String>,
    current_dir: Option<PathBuf>,
}

trait CommandRunner: Send + Sync {
    fn run(&self, spec: &CommandSpec, acceptance_timeout: Duration) -> Result<(), String>;
}

#[derive(Debug, Default)]
struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, spec: &CommandSpec, acceptance_timeout: Duration) -> Result<(), String> {
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(current_dir) = &spec.current_dir {
            command.current_dir(current_dir);
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("spawn {}: {error}", spec.program.display()))?;

        let deadline = Instant::now() + acceptance_timeout;
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(()),
                Ok(Some(status)) => {
                    return Err(format!("{} exited with {status}", spec.program.display()));
                }
                Ok(None) if Instant::now() >= deadline => {
                    let _ = thread::Builder::new()
                        .name("rambledesk-host-wakeup".to_owned())
                        .spawn(move || {
                            let _ = child.wait();
                        });
                    return Ok(());
                }
                Ok(None) => thread::sleep(Duration::from_millis(25)),
                Err(error) => {
                    return Err(format!(
                        "inspect {} after spawn: {error}",
                        spec.program.display()
                    ));
                }
            }
        }
    }
}

struct HostWakeupAdapter {
    kind: HostKind,
    runner: Arc<dyn CommandRunner>,
    acceptance_timeout: Duration,
}

impl HostWakeupAdapter {
    fn new(kind: HostKind) -> Self {
        Self {
            kind,
            runner: Arc::new(SystemCommandRunner),
            acceptance_timeout: ACCEPTANCE_TIMEOUT,
        }
    }

    #[cfg(test)]
    fn with_runner(kind: HostKind, runner: Arc<dyn CommandRunner>) -> Self {
        Self {
            kind,
            runner,
            acceptance_timeout: Duration::from_millis(1),
        }
    }
}

impl WakeupAdapter for HostWakeupAdapter {
    fn id(&self) -> &'static str {
        self.kind.id()
    }

    fn matches_host(&self, host_id: &str) -> bool {
        let normalized = normalize_host_id(host_id);
        self.kind
            .aliases()
            .iter()
            .any(|alias| normalize_host_id(alias) == normalized)
    }

    fn wake(&self, payload: &WakePayload) -> WakeResult {
        if payload.session_id.trim().is_empty() {
            return GenericWakeupAdapter.wake(payload);
        }

        for spec in self.kind.command_specs(payload) {
            if self.runner.run(&spec, self.acceptance_timeout).is_ok() {
                return WakeResult::HostDelivered {
                    adapter_id: self.id().to_owned(),
                    host_id: payload.host_id.clone(),
                };
            }
        }

        GenericWakeupAdapter.wake(payload)
    }
}

fn normalize_host_id(host_id: &str) -> String {
    host_id
        .trim()
        .chars()
        .filter(|character| !matches!(character, '-' | '_' | ' '))
        .flat_map(char::to_lowercase)
        .collect()
}

fn binary_candidates(env_key: &str, fallbacks: &[impl AsRef<std::path::Path>]) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(value) = env::var_os(env_key).filter(|value| !value.is_empty()) {
        candidates.push(PathBuf::from(value));
    }
    for fallback in fallbacks {
        candidates.push(fallback.as_ref().to_path_buf());
    }
    dedupe_paths(candidates)
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut unique = Vec::with_capacity(paths.len());
    for path in paths {
        if !unique.iter().any(|candidate| candidate == &path) {
            unique.push(path);
        }
    }
    unique
}

fn project_current_dir(payload: &WakePayload) -> Option<PathBuf> {
    payload
        .project_root_path
        .as_deref()
        .and_then(clean)
        .map(PathBuf::from)
}

fn clean(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn claude_command_specs(payload: &WakePayload, home: Option<&Path>) -> Vec<CommandSpec> {
    let prompt = payload.resume_prompt();
    let resume_context = claude_resume_context(payload, home);
    let current_dir = project_current_dir(payload);
    binary_candidates("RAMBLEDESK_CLAUDE_BIN", ["claude"].as_slice())
        .into_iter()
        .map(|program| {
            let mut args = vec![
                "--resume".to_owned(),
                resume_context.session_id.clone(),
                "-p".to_owned(),
                "--output-format".to_owned(),
                "json".to_owned(),
            ];
            match resume_context.permission_mode {
                ClaudePermissionMode::BypassPermissions => {
                    args.extend([
                        "--permission-mode".to_owned(),
                        "bypassPermissions".to_owned(),
                    ]);
                }
                ClaudePermissionMode::Default => {
                    args.extend([
                        "--allowedTools".to_owned(),
                        "mcp__rambledesk__get_feedback".to_owned(),
                        "--permission-mode".to_owned(),
                        "dontAsk".to_owned(),
                    ]);
                }
            }
            args.push(prompt.clone());
            CommandSpec {
                program,
                args,
                current_dir: current_dir.clone(),
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClaudeResumeContext {
    session_id: String,
    permission_mode: ClaudePermissionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClaudePermissionMode {
    BypassPermissions,
    Default,
}

impl ClaudePermissionMode {
    fn from_str(value: &str) -> Self {
        match value {
            "bypassPermissions" => Self::BypassPermissions,
            _ => Self::Default,
        }
    }
}

fn claude_resume_context(payload: &WakePayload, home: Option<&Path>) -> ClaudeResumeContext {
    home.and_then(|home| infer_claude_resume_context_from_home(home, payload))
        .unwrap_or_else(|| ClaudeResumeContext {
            session_id: payload.session_id.trim().to_owned(),
            permission_mode: ClaudePermissionMode::Default,
        })
}

fn infer_claude_resume_context_from_home(
    home: &Path,
    payload: &WakePayload,
) -> Option<ClaudeResumeContext> {
    let projects_dir = home.join(".claude").join("projects");
    let request_id = clean(&payload.request_id)?;
    let mut candidate_dirs = Vec::new();
    if let Some(project_root) = payload
        .project_root_path
        .as_deref()
        .and_then(clean)
        .map(PathBuf::from)
    {
        candidate_dirs.push(projects_dir.join(claude_project_dir_name(&project_root)));
        if let Ok(canonical) = fs::canonicalize(&project_root) {
            candidate_dirs.push(projects_dir.join(claude_project_dir_name(&canonical)));
        }
    }
    candidate_dirs = dedupe_paths(candidate_dirs);

    for directory in &candidate_dirs {
        if let Some(context) = find_claude_resume_context_in_project_dir(directory, request_id) {
            return Some(context);
        }
    }

    for directory in fs::read_dir(projects_dir).ok()?.flatten() {
        let path = directory.path();
        if candidate_dirs.iter().any(|candidate| candidate == &path) {
            continue;
        }
        if let Some(context) = find_claude_resume_context_in_project_dir(&path, request_id) {
            return Some(context);
        }
    }
    None
}

fn claude_project_dir_name(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|character| match character {
            '/' | '\\' => '-',
            character
                if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') =>
            {
                character
            }
            _ => '-',
        })
        .collect()
}

fn find_claude_resume_context_in_project_dir(
    directory: &Path,
    request_id: &str,
) -> Option<ClaudeResumeContext> {
    if !directory.is_dir() {
        return None;
    }
    let mut files = fs::read_dir(directory)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    files.reverse();

    for path in files {
        let Ok(file) = fs::File::open(path) else {
            continue;
        };
        let mut match_in_file = None;
        let mut permission_mode = ClaudePermissionMode::Default;
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            if let Some(mode) = value
                .get("permissionMode")
                .and_then(serde_json::Value::as_str)
            {
                permission_mode = ClaudePermissionMode::from_str(mode);
            }
            if !line.contains(request_id) {
                continue;
            }
            if !claude_json_references_feedback_request(&value, request_id) {
                continue;
            }
            if let Some(session_id) = claude_session_id_from_json(&value) {
                match_in_file = Some(ClaudeResumeContext {
                    session_id,
                    permission_mode,
                });
            }
        }
        if match_in_file.is_some() {
            return match_in_file;
        }
    }
    None
}

fn claude_json_references_feedback_request(value: &serde_json::Value, request_id: &str) -> bool {
    if value
        .pointer("/mcpMeta/structuredContent/request_id")
        .and_then(serde_json::Value::as_str)
        == Some(request_id)
    {
        return true;
    }
    if value
        .get("attributionMcpTool")
        .and_then(serde_json::Value::as_str)
        == Some("request_feedback")
        && json_value_contains(value, request_id)
    {
        return true;
    }
    if let Some(result) = value
        .get("toolUseResult")
        .and_then(serde_json::Value::as_str)
        && serde_json::from_str::<serde_json::Value>(result)
            .ok()
            .and_then(|parsed| {
                parsed
                    .get("request_id")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .as_deref()
            == Some(request_id)
    {
        return true;
    }

    value
        .pointer("/message/content")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|blocks| {
            blocks.iter().any(|block| {
                block.get("name").and_then(serde_json::Value::as_str)
                    == Some("mcp__rambledesk__request_feedback")
                    && json_value_contains(block.get("input").unwrap_or(block), request_id)
            })
        })
}

fn claude_session_id_from_json(value: &serde_json::Value) -> Option<String> {
    value
        .get("sessionId")
        .or_else(|| value.get("session_id"))
        .and_then(serde_json::Value::as_str)
        .and_then(clean)
        .map(ToOwned::to_owned)
}

fn json_value_contains(value: &serde_json::Value, needle: &str) -> bool {
    match value {
        serde_json::Value::String(value) => value.contains(needle),
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| json_value_contains(value, needle)),
        serde_json::Value::Object(values) => values
            .values()
            .any(|value| json_value_contains(value, needle)),
        _ => false,
    }
}

fn pi_args(session_id: &str, prompt: &str) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(session_dir) = env::var("RAMBLEDESK_PI_SESSION_DIR")
        .or_else(|_| env::var("PI_CODING_AGENT_SESSION_DIR"))
        .ok()
        .as_deref()
        .and_then(clean)
    {
        args.extend(["--session-dir".to_owned(), session_dir.to_owned()]);
    }
    args.extend([
        "--session".to_owned(),
        session_id.to_owned(),
        "--print".to_owned(),
        prompt.to_owned(),
    ]);
    args
}

fn opencode_home_binary() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".opencode").join("bin").join("opencode"))
}

fn opencode_server_url() -> Option<String> {
    env::var("RAMBLEDESK_OPENCODE_SERVER_URL")
        .or_else(|_| env::var("OPENCODE_SERVER_URL"))
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct RecordingRunner {
        calls: Mutex<Vec<CommandSpec>>,
        results: Mutex<Vec<Result<(), String>>>,
        fallback: Mutex<Result<(), String>>,
    }

    impl Default for RecordingRunner {
        fn default() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                results: Mutex::new(Vec::new()),
                fallback: Mutex::new(Ok(())),
            }
        }
    }

    impl RecordingRunner {
        fn failing() -> Arc<Self> {
            Arc::new(Self {
                calls: Mutex::new(Vec::new()),
                results: Mutex::new(vec![Err("boom".to_owned())]),
                fallback: Mutex::new(Err("boom".to_owned())),
            })
        }

        fn calls(&self) -> Vec<CommandSpec> {
            self.calls.lock().expect("calls").clone()
        }
    }

    impl CommandRunner for RecordingRunner {
        fn run(&self, spec: &CommandSpec, _acceptance_timeout: Duration) -> Result<(), String> {
            self.calls.lock().expect("calls").push(spec.clone());
            self.results
                .lock()
                .expect("results")
                .pop()
                .unwrap_or_else(|| self.fallback.lock().expect("fallback").clone())
        }
    }

    fn payload(host_id: &str, session_id: &str) -> WakePayload {
        WakePayload {
            request_id: "0195f7e2-5c31-7b5a-8ab7-3c84ea4fc827".to_owned(),
            host_id: host_id.to_owned(),
            agent: host_id.to_owned(),
            session_id: session_id.to_owned(),
            project_root_path: Some("/tmp/rambledesk-project".to_owned()),
            reason: crate::WakeReason::Completed,
        }
    }

    #[test]
    fn known_adapters_cover_target_hosts() {
        let router = crate::WakeupRouter::default();
        for host in ["claude", "claudecode", "codex", "pi", "opencode"] {
            assert_ne!(router.resolve(Some(host)).id(), "generic");
        }
    }

    #[test]
    fn claude_adapter_delivers_resume_to_session() {
        let runner = Arc::new(RecordingRunner::default());
        let adapter = HostWakeupAdapter::with_runner(HostKind::ClaudeCode, runner.clone());
        let result = adapter.wake(&payload("claude", "claude-session"));

        assert_eq!(
            result,
            WakeResult::HostDelivered {
                adapter_id: "claude".to_owned(),
                host_id: "claude".to_owned()
            }
        );
        let calls = runner.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].args[0], "--resume");
        assert_eq!(calls[0].args[1], "claude-session");
        assert!(calls[0].args.contains(&"-p".to_owned()));
        assert!(
            calls[0]
                .args
                .windows(2)
                .any(|args| { args == ["--output-format".to_owned(), "json".to_owned(),] })
        );
        assert!(calls[0].args.windows(2).any(|args| {
            args == [
                "--allowedTools".to_owned(),
                "mcp__rambledesk__get_feedback".to_owned(),
            ]
        }));
        assert!(
            calls[0]
                .args
                .windows(2)
                .any(|args| { args == ["--permission-mode".to_owned(), "dontAsk".to_owned(),] })
        );
        assert!(!calls[0].args.contains(&"--background".to_owned()));
        assert_eq!(
            calls[0].current_dir,
            Some(PathBuf::from("/tmp/rambledesk-project"))
        );
        assert!(calls[0].args.iter().any(|arg| arg.contains("get_feedback")));
    }

    #[test]
    fn claude_adapter_infers_real_session_from_transcript() {
        let home = tempfile::tempdir().expect("home");
        let project_dir = home
            .path()
            .join(".claude")
            .join("projects")
            .join("-tmp-rambledesk-project");
        std::fs::create_dir_all(&project_dir).expect("project dir");
        std::fs::write(
            project_dir.join("real-session.jsonl"),
            r#"{"type":"user","sessionId":"e5ca15ac-8023-487c-a7fb-6448afb1109a","permissionMode":"bypassPermissions","message":{"content":"start"}}"#
                .to_owned()
                + "\n"
                + r#"{"type":"assistant","sessionId":"e5ca15ac-8023-487c-a7fb-6448afb1109a","message":{"content":[{"type":"tool_use","name":"mcp__rambledesk__request_feedback","input":{"session_id":"test-session-001"}}]}}"#
                + "\n"
                + r#"{"type":"user","sessionId":"e5ca15ac-8023-487c-a7fb-6448afb1109a","toolUseResult":"{\"request_id\":\"0195f7e2-5c31-7b5a-8ab7-3c84ea4fc827\"}"}"#,
        )
        .expect("transcript");

        let specs = claude_command_specs(
            &payload("claude-code", "test-session-001"),
            Some(home.path()),
        );

        assert!(specs.iter().any(|spec| {
            spec.args.windows(2).any(|args| {
                args == [
                    "--resume".to_owned(),
                    "e5ca15ac-8023-487c-a7fb-6448afb1109a".to_owned(),
                ]
            })
        }));
        assert!(specs.iter().any(|spec| {
            spec.args.windows(2).any(|args| {
                args == [
                    "--permission-mode".to_owned(),
                    "bypassPermissions".to_owned(),
                ]
            }) && !spec
                .args
                .contains(&"mcp__rambledesk__get_feedback".to_owned())
        }));
    }

    #[test]
    fn codex_adapter_uses_exec_resume() {
        let specs = HostKind::Codex.command_specs(&payload("codex", "codex-session"));
        assert!(specs.iter().any(|spec| {
            spec.args.starts_with(&[
                "exec".to_owned(),
                "resume".to_owned(),
                "codex-session".to_owned(),
            ]) && spec.args.contains(&"--json".to_owned())
        }));
    }

    #[test]
    fn pi_adapter_uses_print_session_resume() {
        let runner = Arc::new(RecordingRunner::default());
        let adapter = HostWakeupAdapter::with_runner(HostKind::Pi, runner.clone());
        let result = adapter.wake(&payload("pi", "pi-session"));

        assert!(matches!(result, WakeResult::HostDelivered { .. }));
        let calls = runner.calls();
        assert_eq!(
            calls[0].current_dir,
            Some(PathBuf::from("/tmp/rambledesk-project"))
        );
        assert!(
            calls[0]
                .args
                .windows(2)
                .any(|args| args == ["--session".to_owned(), "pi-session".to_owned()])
        );
        assert!(calls[0].args.contains(&"--print".to_owned()));
    }

    #[test]
    fn opencode_adapter_uses_run_session() {
        let specs = HostKind::OpenCode.command_specs(&payload("opencode", "ses_123"));
        assert!(specs.iter().any(|spec| {
            spec.args.first() == Some(&"run".to_owned())
                && spec
                    .args
                    .windows(2)
                    .any(|args| args == ["--dir".to_owned(), "/tmp/rambledesk-project".to_owned()])
                && spec
                    .args
                    .windows(2)
                    .any(|args| args == ["--session".to_owned(), "ses_123".to_owned()])
        }));
    }

    #[test]
    fn missing_session_id_falls_back_to_generic_prompt() {
        let runner = Arc::new(RecordingRunner::default());
        let adapter = HostWakeupAdapter::with_runner(HostKind::Codex, runner.clone());
        let result = adapter.wake(&payload("codex", " "));

        assert!(matches!(
            result,
            WakeResult::UserPrompt {
                adapter_id,
                ..
            } if adapter_id == "generic"
        ));
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn command_failure_falls_back_to_generic_prompt() {
        let runner = RecordingRunner::failing();
        let adapter = HostWakeupAdapter::with_runner(HostKind::OpenCode, runner);
        let result = adapter.wake(&payload("opencode", "ses_123"));

        assert!(matches!(
            result,
            WakeResult::UserPrompt {
                adapter_id,
                ..
            } if adapter_id == "generic"
        ));
    }
}
