use std::{
    env,
    path::PathBuf,
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
        match self {
            Self::ClaudeCode => binary_candidates("RAMBLEDESK_CLAUDE_BIN", ["claude"].as_slice())
                .into_iter()
                .map(|program| CommandSpec {
                    program,
                    args: vec![
                        "--resume".to_owned(),
                        session_id.clone(),
                        "--background".to_owned(),
                        prompt.clone(),
                    ],
                })
                .collect(),
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
            })
            .collect(),
            Self::Pi => binary_candidates("RAMBLEDESK_PI_BIN", ["pi"].as_slice())
                .into_iter()
                .map(|program| CommandSpec {
                    program,
                    args: vec![
                        "--session".to_owned(),
                        session_id.clone(),
                        "--print".to_owned(),
                        prompt.clone(),
                    ],
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
                    args.extend(["--session".to_owned(), session_id.clone(), prompt.clone()]);
                    specs.push(CommandSpec { program, args });
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
}

trait CommandRunner: Send + Sync {
    fn run(&self, spec: &CommandSpec, acceptance_timeout: Duration) -> Result<(), String>;
}

#[derive(Debug, Default)]
struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, spec: &CommandSpec, acceptance_timeout: Duration) -> Result<(), String> {
        let mut child = Command::new(&spec.program)
            .args(&spec.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
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
        assert!(calls[0].args.contains(&"--background".to_owned()));
        assert!(calls[0].args.iter().any(|arg| arg.contains("get_feedback")));
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
            calls[0].args[0..3],
            [
                "--session".to_owned(),
                "pi-session".to_owned(),
                "--print".to_owned()
            ]
        );
    }

    #[test]
    fn opencode_adapter_uses_run_session() {
        let specs = HostKind::OpenCode.command_specs(&payload("opencode", "ses_123"));
        assert!(specs.iter().any(|spec| {
            spec.args.first() == Some(&"run".to_owned())
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
