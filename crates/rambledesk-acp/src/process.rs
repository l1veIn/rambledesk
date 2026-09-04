use std::{collections::BTreeMap, path::Path, process::Stdio, time::Duration};
use tokio::{
    io::AsyncReadExt,
    process::{Child, Command},
};

use crate::AcpError;

pub(crate) fn spawn(
    command: &str,
    args: &[String],
    env: &BTreeMap<String, String>,
    cwd: &Path,
) -> Result<Child, AcpError> {
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
    #[cfg(windows)]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW: npm shims must not open a console.
    #[cfg(unix)]
    cmd.process_group(0);
    Ok(cmd.spawn()?)
}

pub(crate) async fn drain_stderr(mut stderr: tokio::process::ChildStderr) {
    // Drain continuously to prevent a full pipe from blocking the agent. Raw stderr
    // may contain credentials, so it is deliberately not logged or sent to clients.
    let mut bytes = [0u8; 4096];
    while matches!(stderr.read(&mut bytes).await, Ok(n) if n > 0) {}
}

pub(crate) async fn reap(child: &mut Child) -> Result<(), AcpError> {
    match tokio::time::timeout(Duration::from_secs(2), child.wait()).await {
        Ok(result) => {
            result?;
        }
        Err(_) => {
            child.kill().await?;
            child.wait().await?;
        }
    }
    Ok(())
}
