// Adapted from Codeg 3ebdfed commands/acp.rs run_npm_streaming and bounded
// version probes (Apache-2.0). Changed: shared owned process trees, cancellation,
// capped lossy output, and safe diagnostic categories instead of raw logs.
use std::{collections::BTreeMap, path::Path, time::Duration};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio_util::sync::CancellationToken;

use super::CatalogError;

#[derive(Clone)]
pub(super) struct CommandSpec {
    pub command: String,
    pub args: Vec<String>,
}
pub(super) struct Output {
    pub stdout: String,
    pub stderr: String,
}

async fn capture(mut pipe: impl AsyncRead + Unpin) -> std::io::Result<String> {
    let mut result = Vec::new();
    let mut bytes = [0u8; 4096];
    loop {
        let count = pipe.read(&mut bytes).await?;
        if count == 0 {
            break;
        }
        let retained = count.min((32 * 1024usize).saturating_sub(result.len()));
        result.extend_from_slice(&bytes[..retained]);
    }
    Ok(String::from_utf8_lossy(&result).into_owned())
}

pub(super) async fn run(
    command: &CommandSpec,
    args: &[String],
    cwd: &Path,
    env: &BTreeMap<String, String>,
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<Output, CatalogError> {
    if cancel.is_cancelled() {
        return Err(CatalogError::Cancelled);
    }
    let args: Vec<_> = command.args.iter().chain(args).cloned().collect();
    let mut child = crate::process::spawn(&command.command, &args, env, cwd)
        .map_err(|_| CatalogError::CommandUnavailable)?;
    drop(child.take_stdin());
    let stdout = child
        .take_stdout()
        .ok_or(CatalogError::CommandUnavailable)?;
    let stderr = child
        .take_stderr()
        .ok_or(CatalogError::CommandUnavailable)?;
    let result = tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            let _ = child.kill_and_reap().await;
            return Err(CatalogError::Cancelled);
        },
        result = async { tokio::join!(child.wait_with_timeout(timeout), capture(stdout), capture(stderr)) } => result,
    };
    let (status, stdout, stderr) = result;
    let status = status
        .map_err(|_| CatalogError::CommandFailed)?
        .ok_or(CatalogError::Timeout)?;
    let stdout = stdout.map_err(|_| CatalogError::CommandFailed)?;
    let stderr = stderr.map_err(|_| CatalogError::CommandFailed)?;
    if !status.success() {
        return Err(if stderr.contains("EACCES") || stderr.contains("EPERM") {
            CatalogError::PermissionDenied
        } else if stderr.contains("ETARGET") || stderr.contains("E404") {
            CatalogError::VersionUnavailable
        } else if stderr.contains("ERR_INVALID_URL") {
            CatalogError::InvalidProxy
        } else {
            CatalogError::CommandFailed
        });
    }
    Ok(Output { stdout, stderr })
}
