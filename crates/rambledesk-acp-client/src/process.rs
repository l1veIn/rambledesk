use std::{
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite},
    process::{Child, Command},
    sync::Mutex,
    time::sleep,
};

use crate::{AcpClientError, AcpErrorCode, LaunchProfile};

pub(crate) type AgentReader = Pin<Box<dyn AsyncRead + Send>>;
pub(crate) type AgentWriter = Pin<Box<dyn AsyncWrite + Send>>;

#[async_trait]
pub(crate) trait ProcessControl: Send + Sync {
    /// Returns true when shutdown had to force the process tree.
    async fn shutdown(&self, grace: Duration) -> Result<bool, AcpClientError>;
}

pub(crate) struct SpawnedAgent {
    pub(crate) reader: AgentReader,
    pub(crate) writer: AgentWriter,
    pub(crate) control: Arc<dyn ProcessControl>,
}

#[async_trait]
pub(crate) trait AgentSpawner: Send + Sync {
    async fn spawn(&self, profile: &LaunchProfile) -> Result<SpawnedAgent, AcpClientError>;
}

#[derive(Debug, Default)]
pub(crate) struct CommandAgentSpawner;

#[async_trait]
impl AgentSpawner for CommandAgentSpawner {
    async fn spawn(&self, profile: &LaunchProfile) -> Result<SpawnedAgent, AcpClientError> {
        if profile.command.as_os_str().is_empty() {
            return Err(AcpClientError::invalid("launch command must not be empty"));
        }
        let mut command = Command::new(&profile.command);
        command
            .args(&profile.args)
            .envs(&profile.env)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        apply_internal_launch_environment(profile, &mut command);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            command.as_std_mut().process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            command
                .as_std_mut()
                .creation_flags(windows_sys::Win32::System::Threading::CREATE_NEW_PROCESS_GROUP);
        }
        let mut child = command.spawn().map_err(|error| {
            AcpClientError::new(
                AcpErrorCode::AgentLaunchFailed,
                format!(
                    "failed to launch {}: {error}",
                    profile.command.to_string_lossy()
                ),
                true,
            )
        })?;
        let pid = child.id();
        let stdin = child.stdin.take().ok_or_else(|| {
            AcpClientError::new(
                AcpErrorCode::AgentLaunchFailed,
                "agent stdin was unavailable",
                true,
            )
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AcpClientError::new(
                AcpErrorCode::AgentLaunchFailed,
                "agent stdout was unavailable",
                true,
            )
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            AcpClientError::new(
                AcpErrorCode::AgentLaunchFailed,
                "agent stderr was unavailable",
                true,
            )
        })?;
        let profile_id = profile.profile_ref.launch_profile_id.clone();
        tokio::spawn(async move {
            drain_stderr(stderr, &profile_id).await;
        });
        Ok(SpawnedAgent {
            reader: Box::pin(stdout),
            writer: Box::pin(stdin),
            control: Arc::new(ChildProcessControl {
                child: Mutex::new(child),
                pid,
            }),
        })
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct StderrStats {
    bytes: u64,
    chunks: u64,
    read_error_kind: Option<std::io::ErrorKind>,
}

async fn drain_stderr(mut stderr: impl AsyncRead + Unpin, profile_id: &str) -> StderrStats {
    let mut stats = StderrStats::default();
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        match stderr.read(&mut buffer).await {
            Ok(0) => break,
            Ok(count) => {
                stats.bytes = stats.bytes.saturating_add(count as u64);
                stats.chunks = stats.chunks.saturating_add(1);
            }
            Err(error) => {
                stats.read_error_kind = Some(error.kind());
                break;
            }
        }
    }
    tracing::debug!(
        launch_profile_id = profile_id,
        stderr_bytes = stats.bytes,
        stderr_chunks = stats.chunks,
        stderr_read_error_kind = ?stats.read_error_kind,
        "ACP Agent stderr stream closed"
    );
    stats
}

fn apply_internal_launch_environment(profile: &LaunchProfile, command: &mut Command) {
    if profile.profile_ref.agent_profile_id == "codex" {
        // Codex ACP otherwise filters a same-named per-Session MCP server
        // against the user's static config. This invariant is owned by the
        // Adapter and intentionally overrides profile/user environment input.
        command.env("DISABLE_MCP_CONFIG_FILTERING", "true");
    }
}

struct ChildProcessControl {
    child: Mutex<Child>,
    pid: Option<u32>,
}

#[async_trait]
impl ProcessControl for ChildProcessControl {
    async fn shutdown(&self, grace: Duration) -> Result<bool, AcpClientError> {
        if wait_until_exit(&self.child, grace).await? {
            return Ok(false);
        }

        terminate_process_tree(self.pid, false).await;
        if wait_until_exit(&self.child, grace.min(Duration::from_millis(750))).await? {
            return Ok(false);
        }

        terminate_process_tree(self.pid, true).await;
        let mut child = self.child.lock().await;
        let _ = child.start_kill();
        let _ = tokio::time::timeout(Duration::from_secs(1), child.wait()).await;
        Ok(true)
    }
}

async fn wait_until_exit(child: &Mutex<Child>, timeout: Duration) -> Result<bool, AcpClientError> {
    let deadline = Instant::now() + timeout;
    loop {
        let status = child.lock().await.try_wait().map_err(|error| {
            AcpClientError::new(
                AcpErrorCode::ShutdownFailed,
                format!("failed to inspect ACP Agent process: {error}"),
                true,
            )
        })?;
        if status.is_some() {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        sleep(Duration::from_millis(20)).await;
    }
}

#[cfg(unix)]
async fn terminate_process_tree(pid: Option<u32>, force: bool) {
    let Some(pid) = pid.and_then(|value| rustix::process::Pid::from_raw(value.cast_signed()))
    else {
        return;
    };
    let signal = if force {
        rustix::process::Signal::KILL
    } else {
        rustix::process::Signal::TERM
    };
    let _ = rustix::process::kill_process_group(pid, signal);
}

#[cfg(windows)]
async fn terminate_process_tree(pid: Option<u32>, _force: bool) {
    let Some(pid) = pid else { return };
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status()
        .await;
}

#[cfg(not(any(unix, windows)))]
async fn terminate_process_tree(_pid: Option<u32>, _force: bool) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LaunchProfile;
    use tokio::io::AsyncWriteExt as _;

    #[test]
    fn codex_internal_environment_cannot_be_overridden_by_profile_input() {
        let mut profile = LaunchProfile::codex_npx();
        profile.env.insert(
            "DISABLE_MCP_CONFIG_FILTERING".to_string(),
            "false".to_string(),
        );
        let mut command = Command::new("ignored");
        command.envs(&profile.env);
        apply_internal_launch_environment(&profile, &mut command);
        let value = command
            .as_std()
            .get_envs()
            .find(|(name, _)| *name == "DISABLE_MCP_CONFIG_FILTERING")
            .and_then(|(_, value)| value)
            .unwrap();
        assert_eq!(value, "true");
    }

    #[tokio::test]
    async fn stderr_is_reduced_to_counts_without_retaining_agent_output() {
        let secret = b"private agent diagnostics must never enter default logs";
        let (mut writer, reader) = tokio::io::duplex(256);
        let task = tokio::spawn(async move { drain_stderr(reader, "profile").await });
        writer.write_all(secret).await.unwrap();
        writer.shutdown().await.unwrap();
        let stats = task.await.unwrap();
        assert_eq!(stats.bytes, secret.len() as u64);
        assert!(stats.chunks > 0);
        assert_eq!(stats.read_error_kind, None);
    }
}
