use std::{collections::BTreeMap, path::Path, process::Stdio, time::Duration};
use tokio::{
    io::AsyncReadExt,
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command},
};

use crate::AcpError;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
use unix::Ownership;
#[cfg(windows)]
use windows::Ownership;

/// A process and the OS resource that owns its descendants. The Child is private:
/// on Unix, nobody may reap the leader before its process group is cleaned up.
pub(crate) struct OwnedProcess {
    child: Child,
    ownership: Ownership,
}

pub(crate) fn spawn(
    command: &str,
    args: &[String],
    env: &BTreeMap<String, String>,
    cwd: &Path,
) -> Result<OwnedProcess, AcpError> {
    if !cwd.is_absolute() || !cwd.is_dir() {
        return Err(AcpError::InvalidLaunch(
            "cwd must be an existing absolute directory".into(),
        ));
    }
    let executable = if Path::new(command).is_absolute() {
        Path::new(command).to_path_buf()
    } else {
        rambledesk_core::find_executable(command).ok_or_else(|| {
            AcpError::InvalidLaunch("agent executable was not found on PATH".into())
        })?
    };
    let mut cmd = Command::new(executable);
    cmd.args(args)
        .envs(env)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // Windows enters its Job Object before its suspended primary thread resumes.
    // Unix creates a separate process group inside spawn.
    let (child, ownership) = Ownership::spawn(cmd)?;
    Ok(OwnedProcess { child, ownership })
}

impl OwnedProcess {
    pub(crate) fn id(&self) -> Option<u32> {
        self.child.id()
    }
    pub(crate) fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
    }
    pub(crate) fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }
    pub(crate) fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    pub(crate) async fn kill_and_reap(&mut self) -> Result<(), AcpError> {
        self.ownership.terminate()?;
        self.child.wait().await?;
        Ok(())
    }

    /// Observe the exit status before cleaning the owned tree. A timeout never
    /// leaves an installer or version probe running in the background.
    pub(crate) async fn wait_with_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<std::process::ExitStatus>, AcpError> {
        self.child.stdin.take();
        let exited = self
            .ownership
            .wait_before_cleanup(&mut self.child, timeout)
            .await?;
        self.ownership.terminate()?;
        let status = self.child.wait().await?;
        Ok(exited.then_some(status))
    }

    async fn reap_with_grace(&mut self, grace: Duration) -> Result<(), AcpError> {
        // Usually moved into the protocol connection; closing a still-owned stdin
        // also makes this operation useful during failed setup.
        self.wait_with_timeout(grace).await.map(|_| ())
    }
}

impl Drop for OwnedProcess {
    fn drop(&mut self) {
        // Runs before Child::drop. The Unix leader still pins its process group;
        // Windows uses a Job HANDLE, never a PID reconstructed from storage.
        let _ = self.ownership.terminate();
    }
}

pub(crate) async fn drain_stderr(mut stderr: ChildStderr) {
    // Drain continuously to prevent a full pipe from blocking the agent. Raw stderr
    // may contain credentials, so it is deliberately not logged or sent to clients.
    let mut bytes = [0u8; 4096];
    while matches!(stderr.read(&mut bytes).await, Ok(n) if n > 0) {}
}

pub(crate) async fn reap(process: &mut OwnedProcess) -> Result<(), AcpError> {
    process.reap_with_grace(Duration::from_secs(2)).await
}

#[cfg(test)]
mod tests;
