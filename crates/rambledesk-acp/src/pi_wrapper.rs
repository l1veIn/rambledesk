//! Pi ACP 0.0.33 has a command override, but does not forward extension arguments
//! or MCP servers. This wrapper adds one explicitly selected managed extension
//! without editing Pi/project settings. It shares RambleDesk's process ownership.
use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

pub const WRAPPER_ENV: &str = "RAMBLEDESK_MANAGED_PI_WRAPPER";
pub const COMMAND_ENV: &str = "RAMBLEDESK_MANAGED_PI_COMMAND";
pub const ARGS_ENV: &str = "RAMBLEDESK_MANAGED_PI_ARGS";
pub const EXTENSION_ENV: &str = "RAMBLEDESK_MANAGED_PI_EXTENSION";
const CONTROL_ENV: &[&str] = &[
    WRAPPER_ENV,
    COMMAND_ENV,
    ARGS_ENV,
    EXTENSION_ENV,
    "PI_ACP_PI_COMMAND",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PiWrapperError;
impl fmt::Display for PiWrapperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Managed Pi wrapper could not start or continue its private RPC process")
    }
}
impl std::error::Error for PiWrapperError {}

// No Debug: this is launch material, not application diagnostics.
pub struct PiNativeLaunch {
    pub command: String,
    pub args: Vec<String>,
}
/// Recognize the installed bridge by package metadata and its actual entry point.
/// A matching executable name alone does not claim managed-feedback support.
pub async fn is_pi_acp_recipe(command: &str, args: &[String]) -> bool {
    crate::agents::pi_acp_package_prefix(command, args)
        .await
        .is_some()
}
pub async fn resolve_native_pi(command: Option<&str>) -> Result<PiNativeLaunch, PiWrapperError> {
    let (command, args) = crate::agents::resolve_native_pi(command.unwrap_or("pi"))
        .await
        .map_err(|_| PiWrapperError)?;
    Ok(PiNativeLaunch { command, args })
}

pub async fn resolve_native_pi_for_agent(
    acp_command: &str,
    acp_args: &[String],
    override_command: Option<&str>,
) -> Result<PiNativeLaunch, PiWrapperError> {
    if let Some(command) = override_command {
        return resolve_native_pi(Some(command)).await;
    }
    if let Some((command, args)) =
        crate::agents::resolve_managed_pi_dependency(acp_command, acp_args).await
    {
        return Ok(PiNativeLaunch { command, args });
    }
    resolve_native_pi(None).await
}

/// Publish immutable runtime resources. These files contain no credentials and
/// are kept for existing instances; callers may collect them after all users stop.
pub async fn install_managed_extension(root: &Path) -> Result<PathBuf, PiWrapperError> {
    if !root.is_absolute() || root.parent().is_none() {
        return Err(PiWrapperError);
    }
    tokio::fs::create_dir_all(root)
        .await
        .map_err(|_| PiWrapperError)?;
    let canonical = tokio::fs::canonicalize(root)
        .await
        .map_err(|_| PiWrapperError)?;
    let destination = root.join(uuid::Uuid::now_v7().to_string());
    tokio::fs::create_dir(&destination)
        .await
        .map_err(|_| PiWrapperError)?;
    let result = async {
        tokio::fs::write(
            destination.join("managed.mjs"),
            include_str!("../../../packages/pi-rambledesk/managed.mjs"),
        )
        .await?;
        tokio::fs::write(
            destination.join("managed-client.mjs"),
            include_str!("../../../packages/pi-rambledesk/managed-client.mjs"),
        )
        .await?;
        Ok::<_, std::io::Error>(destination.join("managed.mjs"))
    }
    .await;
    if result.is_err()
        && let Ok(path) = tokio::fs::canonicalize(&destination).await
        && path != canonical
        && path.starts_with(&canonical)
    {
        let _ = tokio::fs::remove_dir_all(path).await;
    }
    result.map_err(|_| PiWrapperError)
}

pub fn process_requested() -> bool {
    std::env::var(WRAPPER_ENV).as_deref() == Ok("1") && {
        let mut args = std::env::args_os().skip(1);
        args.next().is_some_and(|arg| arg == "--mode")
            && args.next().is_some_and(|arg| arg == "rpc")
    }
}

pub struct PiWrapperLaunch {
    pub native: PiNativeLaunch,
    pub extension: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}
impl PiWrapperLaunch {
    pub fn from_env() -> Result<Self, PiWrapperError> {
        if !process_requested() {
            return Err(PiWrapperError);
        }
        let token = std::env::var("RAMBLEDESK_MANAGED_MCP_TOKEN").map_err(|_| PiWrapperError)?;
        if token.len() != 64
            || !token.bytes().all(|byte| byte.is_ascii_hexdigit())
            || std::env::var("RAMBLEDESK_MANAGED_MCP_URL").is_err()
        {
            return Err(PiWrapperError);
        }
        let command = std::env::var(COMMAND_ENV).map_err(|_| PiWrapperError)?;
        let encoded = std::env::var(ARGS_ENV).map_err(|_| PiWrapperError)?;
        if encoded.len() > 64 * 1024 {
            return Err(PiWrapperError);
        }
        let args = serde_json::from_str(&encoded).map_err(|_| PiWrapperError)?;
        let launch = Self {
            native: PiNativeLaunch { command, args },
            extension: std::env::var_os(EXTENSION_ENV)
                .map(PathBuf::from)
                .ok_or(PiWrapperError)?,
            args: std::env::args_os()
                .skip(1)
                .map(|arg| arg.into_string().map_err(|_| PiWrapperError))
                .collect::<Result<Vec<_>, _>>()?,
            cwd: std::env::current_dir().map_err(|_| PiWrapperError)?,
        };
        launch.validate()?;
        Ok(launch)
    }
    fn validate(&self) -> Result<(), PiWrapperError> {
        if !Path::new(&self.native.command).is_absolute()
            || !Path::new(&self.native.command).is_file()
            || !self.extension.is_absolute()
            || !self.extension.is_file()
            || !self.cwd.is_absolute()
            || !self.cwd.is_dir()
            || self.native.args.len() > 128
            || self.native.args.iter().any(|arg| arg.len() > 8192)
            || self.args.first().map(String::as_str) != Some("--mode")
            || self.args.get(1).map(String::as_str) != Some("rpc")
            || self.args.len() > 128
            || self.args.iter().any(|arg| arg.len() > 8192)
        {
            return Err(PiWrapperError);
        }
        // Never execute a command shell or recursively launch this wrapper.
        if Path::new(&self.native.command)
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| {
                matches!(value.to_ascii_lowercase().as_str(), "cmd" | "bat" | "ps1")
            })
            || std::fs::canonicalize(&self.native.command).ok()
                == std::env::current_exe()
                    .ok()
                    .and_then(|path| std::fs::canonicalize(path).ok())
        {
            return Err(PiWrapperError);
        }
        Ok(())
    }
}

pub async fn run<R, W>(
    launch: PiWrapperLaunch,
    mut input: R,
    mut output: W,
) -> Result<i32, PiWrapperError>
where
    R: AsyncRead + Send + Unpin,
    W: AsyncWrite + Send + Unpin,
{
    launch.validate()?;
    let mut args = launch.native.args;
    args.extend(launch.args);
    args.push("--extension".into());
    args.push(crate::agents::command_path(&launch.extension));
    let mut child = crate::process::spawn_filtered(
        &launch.native.command,
        &args,
        &BTreeMap::new(),
        &launch.cwd,
        CONTROL_ENV,
    )
    .map_err(|_| PiWrapperError)?;
    let mut child_input = child.take_stdin().ok_or(PiWrapperError)?;
    let mut child_output = child.take_stdout().ok_or(PiWrapperError)?;
    let child_errors = child.take_stderr().ok_or(PiWrapperError)?;
    let mut input_copy = Box::pin(tokio::io::copy(&mut input, &mut child_input));
    let mut output_copy = Box::pin(tokio::io::copy(&mut child_output, &mut output));
    let mut stderr = Box::pin(crate::process::drain_stderr(child_errors));
    let mut stderr_done = false;
    let mut output_done = false;
    let status = loop {
        tokio::select! {
            result=child.wait_for_exit()=>break Some(result.map_err(|_|PiWrapperError)?),
            _=&mut input_copy=>break None,
            _=&mut output_copy=>{output_done=true;break None},
            _=&mut stderr, if !stderr_done=>{stderr_done=true;},
        }
    };
    drop(input_copy);
    let _ = child_input.shutdown().await;
    drop(child_input);
    let status = match status {
        Some(status) => Some(status),
        None => child
            .wait_with_timeout(Duration::from_secs(2))
            .await
            .map_err(|_| PiWrapperError)?,
    };
    // The process guard has closed every owned writer. Flush remaining protocol
    // bytes with a bound; stderr is drained and never copied into Agent output.
    if !output_done {
        let _ = tokio::time::timeout(Duration::from_secs(2), &mut output_copy).await;
    }
    if !stderr_done {
        let _ = tokio::time::timeout(Duration::from_secs(2), &mut stderr).await;
    }
    Ok(status.and_then(|status| status.code()).unwrap_or(1))
}

pub fn run_process() -> i32 {
    let result = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|_| PiWrapperError)
        .and_then(|runtime| {
            let result = runtime.block_on(async {
                run(
                    PiWrapperLaunch::from_env()?,
                    tokio::io::stdin(),
                    tokio::io::stdout(),
                )
                .await
            });
            runtime.shutdown_timeout(Duration::from_millis(100));
            result
        });
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}
