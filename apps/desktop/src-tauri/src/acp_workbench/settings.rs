use std::{
    collections::{BTreeMap, HashMap},
    env, fs,
    io::{self, Write as _},
    path::{Path, PathBuf},
    time::Duration,
};

use rambledesk_acp_client::{
    BuiltinAgentDistribution, BuiltinAgentSpec, LaunchProfile, PlatformArtifact, builtin_agents,
};
use rambledesk_hosts::host_profile;
use tokio::sync::Mutex;

use self::installer::{install_binary, install_io_error, install_npm, validate_binary_install};
use super::model::{AcpWorkbenchError, AgentSummary};

mod installer;

const PREPARE_TIMEOUT: Duration = Duration::from_secs(180);

/// Fixed, release-owned ACP Agent catalog and managed-runtime installer.
///
/// Profiles are resolved once when the Desktop starts. Missing clients point at
/// their final managed path, so installing them later does not mutate the ACP
/// orchestration graph or leak commands into product state.
pub(super) struct AcpRuntimeCatalog {
    runtimes: HashMap<String, RuntimeEntry>,
    prepare_lock: Mutex<()>,
}

struct RuntimeEntry {
    profile: LaunchProfile,
    source: RuntimeSource,
}

enum RuntimeSource {
    System {
        node_minimum: Option<&'static str>,
    },
    ManagedNpm {
        prefix: PathBuf,
        package: &'static str,
        node_minimum: &'static str,
    },
    ManagedBinary {
        install_dir: PathBuf,
        executable: PathBuf,
        artifact: &'static PlatformArtifact,
        spec: &'static BuiltinAgentSpec,
    },
    Unsupported(AcpWorkbenchError),
}

impl AcpRuntimeCatalog {
    pub(super) fn open(v3_root: &Path) -> Result<Self, AcpWorkbenchError> {
        let directories = executable_directories();
        let path_env = augmented_path(&directories);
        let mut runtimes = HashMap::new();
        for spec in builtin_agents() {
            let (command, source) = resolve_runtime(v3_root, spec, &directories)?;
            let mut extra_env = BTreeMap::new();
            extra_env.insert("PATH".to_owned(), path_env.clone());
            let (args, distribution_env) = match spec.distribution {
                BuiltinAgentDistribution::Npm { args, env, .. }
                | BuiltinAgentDistribution::Binary { args, env, .. } => (args, env),
            };
            for (name, value) in distribution_env {
                extra_env.insert((*name).to_owned(), (*value).to_owned());
            }
            let profile = LaunchProfile::for_builtin(
                spec,
                command,
                args.iter().map(|value| (*value).to_owned()).collect(),
                extra_env,
            );
            runtimes.insert(spec.id.to_owned(), RuntimeEntry { profile, source });
        }
        Ok(Self {
            runtimes,
            prepare_lock: Mutex::new(()),
        })
    }

    pub(super) fn agents(&self) -> Vec<AgentSummary> {
        builtin_agents()
            .iter()
            .map(|spec| AgentSummary {
                id: spec.id.to_owned(),
                label: spec.label.to_owned(),
                icon_svg: host_profile(host_profile_id(spec.id)).icon_svg,
                supports_structured_ramble: spec.supports_session_mcp,
                models: Vec::new(),
                reasoning_efforts: Vec::new(),
            })
            .collect()
    }

    pub(super) fn runtime_profiles(&self) -> Vec<LaunchProfile> {
        builtin_agents()
            .iter()
            .filter_map(|spec| self.runtimes.get(spec.id))
            .map(|runtime| runtime.profile.clone())
            .collect()
    }

    pub(super) fn launch_profile_id(&self, agent_id: &str) -> Option<String> {
        self.runtimes
            .get(agent_id)
            .map(|runtime| runtime.profile.profile_ref.launch_profile_id.clone())
    }

    pub(super) fn runtime_error(&self, agent_id: &str) -> Option<AcpWorkbenchError> {
        let Some(runtime) = self.runtimes.get(agent_id) else {
            return Some(unsupported_agent(agent_id));
        };
        match &runtime.source {
            RuntimeSource::Unsupported(error) => Some(error.clone()),
            RuntimeSource::ManagedNpm { node_minimum, .. } => {
                validate_node_runtime(node_minimum).err()
            }
            RuntimeSource::System { node_minimum } => {
                node_minimum.and_then(|minimum| validate_node_runtime(minimum).err())
            }
            RuntimeSource::ManagedBinary { .. } => None,
        }
    }

    pub(super) async fn prepare(&self, agent_id: &str) -> Result<(), AcpWorkbenchError> {
        tokio::time::timeout(PREPARE_TIMEOUT, self.prepare_without_timeout(agent_id))
            .await
            .map_err(|_| {
                AcpWorkbenchError::new(
                    "ACP_OPERATION_TIMED_OUT",
                    "preparing the pinned Agent client exceeded three minutes",
                    true,
                )
            })?
    }

    async fn prepare_without_timeout(&self, agent_id: &str) -> Result<(), AcpWorkbenchError> {
        let _guard = self.prepare_lock.lock().await;
        if let Some(error) = self.runtime_error(agent_id) {
            return Err(error);
        }
        if agent_id == "antigravity" {
            ensure_antigravity_auth_settings()?;
        }
        let runtime = self
            .runtimes
            .get(agent_id)
            .ok_or_else(|| unsupported_agent(agent_id))?;
        match &runtime.source {
            RuntimeSource::System { .. } => Ok(()),
            RuntimeSource::Unsupported(error) => Err(error.clone()),
            RuntimeSource::ManagedNpm {
                prefix,
                package,
                node_minimum,
            } => {
                validate_node_runtime(node_minimum)?;
                if runtime.profile.command.is_file() {
                    return Ok(());
                }
                install_npm(prefix, package, &runtime.profile.command).await
            }
            RuntimeSource::ManagedBinary {
                install_dir,
                executable,
                artifact,
                spec,
            } => {
                if validate_binary_install(executable, install_dir, spec).is_ok() {
                    return Ok(());
                }
                install_binary(install_dir, executable, artifact, spec).await
            }
        }
    }
}

fn resolve_runtime(
    v3_root: &Path,
    spec: &'static BuiltinAgentSpec,
    directories: &[PathBuf],
) -> Result<(PathBuf, RuntimeSource), AcpWorkbenchError> {
    match spec.distribution {
        BuiltinAgentDistribution::Npm {
            version,
            package,
            command,
            node_minimum,
            ..
        } => {
            if let Some(system) = resolve_pinned_npm_command(command, version, directories) {
                return Ok((
                    system,
                    RuntimeSource::System {
                        node_minimum: Some(node_minimum),
                    },
                ));
            }
            let prefix = v3_root.join("acp-clients/npm").join(spec.id).join(version);
            let executable = npm_prefix_command(&prefix, command, package);
            Ok((
                executable,
                RuntimeSource::ManagedNpm {
                    prefix,
                    package,
                    node_minimum,
                },
            ))
        }
        BuiltinAgentDistribution::Binary {
            version,
            command,
            artifacts,
            directory_entry,
            ..
        } => {
            // Antigravity's IDE bundle contains a same-named internal binary
            // with a different directory contract; only our managed tree is safe.
            if spec.id != "antigravity"
                && let Some(system) = resolve_executable_from(command, directories)
            {
                return Ok((system, RuntimeSource::System { node_minimum: None }));
            }
            let Some(artifact) = artifact_for_current_platform(artifacts) else {
                let error = AcpWorkbenchError::new(
                    "ACP_PLATFORM_UNSUPPORTED",
                    format!("{} is not distributed for {}", spec.label, platform_key()),
                    false,
                );
                return Ok((PathBuf::new(), RuntimeSource::Unsupported(error)));
            };
            let install_dir = v3_root
                .join("acp-clients/binary")
                .join(spec.id)
                .join(version);
            let relative = directory_entry
                .map(platform_entry)
                .unwrap_or_else(|| platform_command_name(command));
            let executable = install_dir.join(relative);
            Ok((
                executable.clone(),
                RuntimeSource::ManagedBinary {
                    install_dir,
                    executable,
                    artifact,
                    spec,
                },
            ))
        }
    }
}

fn validate_node_runtime(minimum: &str) -> Result<(), AcpWorkbenchError> {
    let node = resolve_executable("node").ok_or_else(|| {
        AcpWorkbenchError::new(
            "ACP_RUNTIME_MISSING",
            format!("Node.js {minimum} or newer is required"),
            false,
        )
    })?;
    let output = std::process::Command::new(node)
        .arg("--version")
        .output()
        .map_err(|error| {
            AcpWorkbenchError::new(
                "ACP_RUNTIME_MISSING",
                format!("could not inspect Node.js: {error}"),
                false,
            )
        })?;
    let actual = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() || !version_at_least(actual.trim(), minimum) {
        return Err(AcpWorkbenchError::new(
            "ACP_RUNTIME_MISSING",
            format!("Node.js {minimum} or newer is required"),
            false,
        ));
    }
    Ok(())
}

fn version_at_least(actual: &str, minimum: &str) -> bool {
    let parse = |value: &str| {
        value
            .trim_start_matches('v')
            .split('.')
            .take(3)
            .map(|part| part.split('-').next().unwrap_or(part).parse::<u64>())
            .collect::<Result<Vec<_>, _>>()
            .ok()
    };
    match (parse(actual), parse(minimum)) {
        (Some(mut actual), Some(mut minimum)) => {
            actual.resize(3, 0);
            minimum.resize(3, 0);
            actual >= minimum
        }
        _ => false,
    }
}

fn resolve_executable(command: &str) -> Option<PathBuf> {
    resolve_executable_from(command, &executable_directories())
}

fn resolve_executable_from(command: &str, directories: &[PathBuf]) -> Option<PathBuf> {
    let executable = platform_command_name(command);
    directories
        .iter()
        .map(|directory| directory.join(&executable))
        .find(|candidate| candidate.is_file())
}

fn resolve_pinned_npm_command(
    command: &str,
    pinned_version: &str,
    directories: &[PathBuf],
) -> Option<PathBuf> {
    let executable = resolve_executable_from(command, directories)?;
    (installed_npm_version(&executable).as_deref() == Some(pinned_version)).then_some(executable)
}

fn installed_npm_version(executable: &Path) -> Option<String> {
    let resolved = fs::canonicalize(executable).ok()?;
    for ancestor in resolved.ancestors().take(8) {
        let package_json = ancestor.join("package.json");
        let Ok(contents) = fs::read_to_string(package_json) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) else {
            continue;
        };
        if let Some(version) = value.get("version").and_then(serde_json::Value::as_str) {
            return Some(version.to_owned());
        }
    }
    None
}

fn executable_directories() -> Vec<PathBuf> {
    let mut directories = env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default();
    if cfg!(target_os = "macos") {
        directories.extend([
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
        ]);
    }
    if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
        directories.extend([
            home.join(".volta/bin"),
            home.join(".asdf/shims"),
            home.join(".local/share/mise/shims"),
            home.join(".local/bin"),
        ]);
        if let Ok(entries) = fs::read_dir(home.join(".nvm/versions/node")) {
            let mut versions = entries
                .flatten()
                .map(|entry| entry.path().join("bin"))
                .collect::<Vec<_>>();
            versions.sort_by(|left, right| right.cmp(left));
            directories.extend(versions);
        }
    }
    let mut unique = Vec::new();
    for directory in directories {
        if !unique.contains(&directory) {
            unique.push(directory);
        }
    }
    unique
}

fn augmented_path(directories: &[PathBuf]) -> String {
    env::join_paths(directories)
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

fn npm_prefix_command(prefix: &Path, command: &str, package: &str) -> PathBuf {
    // Cline's 3.0.60 postinstall materializes a native payload and leaves its
    // declared bin at the package path without a node_modules/.bin link.
    if package.starts_with("cline@") && !cfg!(windows) {
        return prefix.join("node_modules/cline/bin/cline");
    }
    if cfg!(windows) {
        prefix
            .join("node_modules/.bin")
            .join(format!("{command}.cmd"))
    } else {
        prefix.join("node_modules/.bin").join(command)
    }
}

fn artifact_for_current_platform(
    artifacts: &'static [PlatformArtifact],
) -> Option<&'static PlatformArtifact> {
    let platform = platform_key();
    artifacts
        .iter()
        .find(|artifact| artifact.platform == platform)
}

fn platform_key() -> String {
    let os = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        env::consts::OS
    };
    let arch = env::consts::ARCH;
    format!("{os}-{arch}")
}

fn platform_command_name(command: &str) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(format!("{command}.exe"))
    } else {
        PathBuf::from(command)
    }
}

fn platform_entry(entry: rambledesk_acp_client::BinaryDirectoryEntry) -> PathBuf {
    PathBuf::from(if cfg!(windows) {
        entry.windows
    } else {
        entry.unix
    })
}

fn host_profile_id(agent_id: &str) -> &str {
    match agent_id {
        "claude_code" => "claude",
        "open_code" => "opencode",
        "deepseek" => "dsh",
        other => other,
    }
}

fn unsupported_agent(agent_id: &str) -> AcpWorkbenchError {
    AcpWorkbenchError::new(
        "ACP_LAUNCH_PROFILE_NOT_FOUND",
        format!("No ACP launch profile is available for Agent `{agent_id}`."),
        false,
    )
}

fn ensure_antigravity_auth_settings() -> Result<(), AcpWorkbenchError> {
    let gemini_home = match env::var_os("GEMINI_HOME").filter(|value| !value.is_empty()) {
        Some(value) => {
            let value = PathBuf::from(value);
            if let Ok(relative) = value.strip_prefix("~") {
                env::var_os("HOME")
                    .map(PathBuf::from)
                    .ok_or_else(|| antigravity_settings_error("the home directory is unavailable"))?
                    .join(relative)
            } else {
                value
            }
        }
        None => env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| antigravity_settings_error("the home directory is unavailable"))?
            .join(".gemini"),
    };
    sync_antigravity_personal_auth(&gemini_home.join("antigravity-acp/settings.json"))
}

fn sync_antigravity_personal_auth(path: &Path) -> Result<(), AcpWorkbenchError> {
    let mut settings = match fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str::<serde_json::Value>(&contents).map_err(|error| {
            antigravity_settings_error(format!(
                "Antigravity settings.json is not strict JSON and was left unchanged: {error}"
            ))
        })?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => serde_json::json!({}),
        Err(error) => {
            return Err(antigravity_settings_error(format!(
                "Antigravity settings.json could not be read: {error}"
            )));
        }
    };
    let root = settings.as_object_mut().ok_or_else(|| {
        antigravity_settings_error(
            "Antigravity settings.json is not an object and was left unchanged",
        )
    })?;
    if root
        .get("auth")
        .and_then(serde_json::Value::as_object)
        .and_then(|auth| auth.get("type"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Ok(());
    }
    if root
        .get("auth")
        .is_some_and(|auth| !auth.is_null() && !auth.is_object())
    {
        return Err(antigravity_settings_error(
            "Antigravity settings.json has a non-object auth block and was left unchanged",
        ));
    }
    root.insert(
        "auth".to_owned(),
        serde_json::json!({"type": "oauth-personal"}),
    );
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let parent = target.parent().ok_or_else(|| {
        antigravity_settings_error("Antigravity settings path has no parent directory")
    })?;
    fs::create_dir_all(parent).map_err(install_io_error)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(install_io_error)?;
    serde_json::to_writer_pretty(temporary.as_file_mut(), &settings).map_err(install_io_error)?;
    temporary
        .as_file_mut()
        .write_all(b"\n")
        .map_err(install_io_error)?;
    temporary
        .as_file_mut()
        .sync_all()
        .map_err(install_io_error)?;
    temporary
        .persist(target)
        .map_err(|error| install_io_error(error.error))?;
    Ok(())
}

fn antigravity_settings_error(message: impl Into<String>) -> AcpWorkbenchError {
    AcpWorkbenchError::new("ACP_AUTHENTICATION_REQUIRED", message, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_exposes_the_complete_codeg_roster() {
        let root = tempfile::tempdir().expect("tempdir");
        let catalog = AcpRuntimeCatalog::open(root.path()).expect("catalog");
        assert_eq!(catalog.agents().len(), 15);
        assert_eq!(catalog.runtime_profiles().len(), 15);
        assert_eq!(
            catalog.launch_profile_id("codex").as_deref(),
            Some("codex-acp-npx")
        );
        assert!(
            !catalog
                .agents()
                .iter()
                .find(|agent| agent.id == "pi")
                .expect("pi")
                .supports_structured_ramble
        );
    }

    #[test]
    fn path_entry_wins_over_managed_install() {
        let root = tempfile::tempdir().expect("tempdir");
        let bin = root.path().join("bin");
        fs::create_dir_all(&bin).expect("bin");
        let executable = bin.join(platform_command_name("codex-acp"));
        fs::write(&executable, "").expect("command");
        assert_eq!(
            resolve_executable_from("codex-acp", &[bin]),
            Some(executable)
        );
    }

    #[test]
    fn semantic_node_versions_are_compared_numerically() {
        assert!(version_at_least("v22.23.0", "22.22.3"));
        assert!(version_at_least("20.0.0", "20.0.0"));
        assert!(!version_at_least("v20.9.0", "22.0.0"));
    }

    #[test]
    fn managed_npm_path_is_release_owned() {
        let root = Path::new("/tmp/rambledesk-v3");
        assert_eq!(
            npm_prefix_command(
                &root.join("acp-clients/npm/codex/1.7.0"),
                "codex-acp",
                "@agentclientprotocol/codex-acp@1.7.0"
            ),
            if cfg!(windows) {
                root.join("acp-clients/npm/codex/1.7.0/node_modules/.bin/codex-acp.cmd")
            } else {
                root.join("acp-clients/npm/codex/1.7.0/node_modules/.bin/codex-acp")
            }
        );
    }

    #[test]
    fn cline_uses_its_postinstall_managed_entrypoint() {
        let root = Path::new("/tmp/cline-managed");
        assert_eq!(
            npm_prefix_command(root, "cline", "cline@3.0.60"),
            root.join("node_modules/cline/bin/cline")
        );
    }

    #[test]
    fn antigravity_auth_defaults_to_personal_oauth_without_overwriting_other_settings() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join("antigravity-acp/settings.json");
        fs::create_dir_all(path.parent().unwrap()).expect("settings directory");
        fs::write(&path, r#"{"keep":{"future":true}}"#).expect("settings");

        sync_antigravity_personal_auth(&path).expect("sync auth");

        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(value["auth"]["type"], "oauth-personal");
        assert_eq!(value["keep"]["future"], true);
    }

    #[test]
    fn antigravity_auth_leaves_foreign_or_unparseable_settings_untouched() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join("settings.json");
        fs::write(&path, "{ // hand-written\n}").expect("settings");

        assert!(sync_antigravity_personal_auth(&path).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "{ // hand-written\n}");
    }

    #[cfg(unix)]
    #[test]
    fn npm_system_command_must_match_the_pinned_package_version() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("tempdir");
        let package = root.path().join("lib/node_modules/example");
        let bin = root.path().join("bin");
        fs::create_dir_all(package.join("dist")).expect("package");
        fs::create_dir_all(&bin).expect("bin");
        fs::write(package.join("package.json"), r#"{"version":"1.2.3"}"#).expect("package json");
        fs::write(package.join("dist/index.js"), "").expect("entry");
        symlink(package.join("dist/index.js"), bin.join("example")).expect("command symlink");

        assert!(
            resolve_pinned_npm_command("example", "1.2.3", std::slice::from_ref(&bin)).is_some()
        );
        assert!(resolve_pinned_npm_command("example", "1.2.4", &[bin]).is_none());
    }

    #[test]
    fn every_runtime_entry_is_backed_by_the_static_catalog() {
        for spec in builtin_agents() {
            assert_eq!(rambledesk_acp_client::builtin_agent(spec.id), Some(spec));
        }
    }
}
