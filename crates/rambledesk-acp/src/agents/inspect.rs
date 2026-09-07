// Adapted from Codeg 3ebdfed acp/preflight.rs and commands/acp.rs (Apache-2.0):
// PATH then npm-prefix lookup, actual installed versions, separate vendor CLI
// evidence. Changed: bounded owned probes, managed-prefix preference, no assumed
// pinned version when a probe fails and no environment/configuration inspection.
use super::{
    AgentCatalogService, CatalogError, catalog, paths,
    runner::{self, CommandSpec},
    version,
};
use rambledesk_core::*;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub(super) struct Toolchain {
    pub node: PathBuf,
    pub npm: Option<CommandSpec>,
    #[cfg(test)]
    pub commands: Option<BTreeMap<String, PathBuf>>,
    #[cfg(test)]
    pub env: BTreeMap<String, String>,
}
impl Toolchain {
    pub fn lookup(&self, command: &str) -> Option<PathBuf> {
        #[cfg(test)]
        if let Some(commands) = &self.commands {
            return commands.get(command).cloned();
        }
        find_executable(command)
    }
    pub fn env(&self) -> BTreeMap<String, String> {
        #[cfg(test)]
        {
            self.env.clone()
        }
        #[cfg(not(test))]
        {
            runtime_environment(&self.node, &[])
        }
    }
}

pub(super) fn runtime_environment(node: &Path, commands: &[PathBuf]) -> BTreeMap<String, String> {
    let original = std::env::var_os("PATH").unwrap_or_default();
    let mut directories = std::env::split_paths(&original).collect::<Vec<_>>();
    let count = directories.len();
    for command in std::iter::once(node).chain(commands.iter().map(PathBuf::as_path)) {
        if command.is_file()
            && let Some(parent) = command.parent().filter(|path| path.is_absolute())
            && !directories.iter().any(|path| path == parent)
        {
            directories.push(parent.to_path_buf());
        }
    }
    if directories.len() != count
        && let Ok(paths) = std::env::join_paths(directories)
        && let Some(paths) = paths.to_str()
    {
        return BTreeMap::from([("PATH".into(), paths.into())]);
    }
    BTreeMap::new()
}

pub(super) fn managed_launch_environment(id: &str, prefix: &Path) -> BTreeMap<String, String> {
    if id != "pi-acp" {
        return BTreeMap::new();
    }
    BTreeMap::from([
        ("PI_ACP_ENABLE_EMBEDDED_CONTEXT".into(), "true".into()),
        (
            "PI_ACP_PI_COMMAND".into(),
            paths::command_path(&prefix.join(if cfg!(windows) {
                "node_modules/.bin/pi.cmd"
            } else {
                "node_modules/.bin/pi"
            })),
        ),
    ])
}

fn node_check(actual: Option<&str>, required: &str) -> AgentCatalogCheck {
    AgentCatalogCheck {
        id: "node".into(),
        status: if actual.is_some_and(|actual| version::meets(actual, required)) {
            AgentCheckStatus::Pass
        } else {
            AgentCheckStatus::Fail
        },
        message: format!(
            "Node.js {} (requires >= {required})",
            actual.unwrap_or("unavailable or version unknown")
        ),
    }
}

async fn package_beside_command(
    path: &Path,
    package: &str,
    command: &str,
) -> Option<(String, PathBuf)> {
    let parent = path.parent()?;
    let canonical = tokio::fs::canonicalize(path).await.ok()?;
    let shim = matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("cmd" | "bat")
    );
    // Recognize npm's standard global and local shim layouts without needing
    // npm to run. A standalone executable with the same name still wins.
    for prefix in [
        parent.to_path_buf(),
        parent.join("../lib"),
        parent.join("../.."),
    ] {
        if let Ok(found) = paths::package_entry(&prefix, package, command).await
            && (shim || found.1 == canonical)
        {
            return Some(found);
        }
    }
    None
}

impl AgentCatalogService {
    async fn lookup_agent(
        &self,
        id: &str,
        command: &str,
        tools: &Toolchain,
        cancel: &CancellationToken,
    ) -> Result<Option<PathBuf>, CatalogError> {
        if let Some(path) = tools.lookup(command) {
            return Ok(Some(path));
        }
        if id != "cursor" {
            return Ok(None);
        }
        let Some(path) = tools.lookup("agent") else {
            return Ok(None);
        };
        let canonical = tokio::fs::canonicalize(&path)
            .await
            .unwrap_or_else(|_| path.clone());
        let cursor_directory = canonical.parent().is_some_and(|parent| {
            parent.components().any(|part| {
                matches!(
                    part.as_os_str()
                        .to_str()
                        .map(str::to_ascii_lowercase)
                        .as_deref(),
                    Some(".cursor" | "cursor-agent")
                )
            })
        });
        if cursor_directory {
            return Ok(Some(path));
        }
        let Ok(launch) = paths::launch(&path, &tools.node).await else {
            return Ok(None);
        };
        // `agent` is a generic name. Do not claim an unrelated program merely
        // because it exists on PATH; require vendor identity from a bounded help probe.
        match runner::run(
            &launch,
            &["--help".into()],
            &std::env::temp_dir(),
            &tools.env(),
            self.probe_timeout,
            cancel,
        )
        .await
        {
            Ok(output)
                if [&output.stdout, &output.stderr].iter().any(|text| {
                    let text = text.to_ascii_lowercase();
                    text.contains("cursor agent")
                        || text.contains("cursor cli")
                        || text.contains("cursor.com")
                }) =>
            {
                Ok(Some(path))
            }
            Err(CatalogError::Cancelled) => Err(CatalogError::Cancelled),
            _ => Ok(None),
        }
    }

    pub(super) async fn tools(&self) -> Toolchain {
        #[cfg(test)]
        if let Some(tools) = &self.tools {
            return tools.clone();
        }
        let node = find_executable("node").unwrap_or_default();
        let npm = if let Some(path) = find_executable("npm") {
            let directory = path.parent().unwrap_or(Path::new(""));
            let cli = [
                directory.join("node_modules/npm/bin/npm-cli.js"),
                directory.join("../lib/node_modules/npm/bin/npm-cli.js"),
            ]
            .into_iter()
            .find(|path| path.is_file())
            .unwrap_or(path);
            paths::launch(&cli, &node).await.ok()
        } else {
            None
        };
        Toolchain {
            node,
            npm,
            #[cfg(test)]
            commands: None,
            #[cfg(test)]
            env: BTreeMap::new(),
        }
    }

    pub(super) async fn probe(
        &self,
        command: &CommandSpec,
        tools: &Toolchain,
        cancel: &CancellationToken,
    ) -> Result<Option<String>, CatalogError> {
        match runner::run(
            command,
            &["--version".into()],
            &std::env::temp_dir(),
            &tools.env(),
            self.probe_timeout,
            cancel,
        )
        .await
        {
            Ok(output) => {
                Ok(version::extract(&output.stdout).or_else(|| version::extract(&output.stderr)))
            }
            Err(CatalogError::Cancelled) => Err(CatalogError::Cancelled),
            Err(_) => Ok(None),
        }
    }

    async fn npm_prefix(
        &self,
        tools: &Toolchain,
        cancel: &CancellationToken,
    ) -> Result<Option<PathBuf>, CatalogError> {
        let Some(npm) = &tools.npm else {
            return Ok(None);
        };
        match runner::run(
            npm,
            &["prefix".into(), "-g".into()],
            &std::env::temp_dir(),
            &tools.env(),
            self.probe_timeout,
            cancel,
        )
        .await
        {
            Ok(output) => Ok(output
                .stdout
                .lines()
                .next()
                .map(str::trim)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute() && path.is_dir())),
            Err(CatalogError::Cancelled) => Err(CatalogError::Cancelled),
            Err(_) => Ok(None),
        }
    }

    pub(super) async fn inspect_inner(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<AgentInspection, CatalogError> {
        let entry = catalog::entry(id).map_err(|_| CatalogError::UnknownAgent)?;
        if cancel.is_cancelled() {
            return Err(CatalogError::Cancelled);
        }
        let tools = self.tools().await;
        let (command, npm) = match &entry.distribution {
            AgentDistribution::Npm {
                command,
                package,
                node_required,
                ..
            } => (
                command.as_str(),
                Some((package.as_str(), node_required.as_str())),
            ),
            AgentDistribution::Manual { command, .. } => (command.as_str(), None),
        };
        let mut checks = vec![];
        let managed = paths::current(&self.root, id).await;
        let mut installed = None;
        let mut source = AgentInstallSource::Missing;
        let mut installed_prefix = None;
        if let (Ok(Some(prefix)), Some((package, _))) = (&managed, npm) {
            if let Ok(found) = paths::package_entry(prefix, package, command).await {
                installed = Some(found);
                source = AgentInstallSource::Managed;
                installed_prefix = Some(prefix.clone());
            } else {
                checks.push(AgentCatalogCheck {
                    id: "managed_integrity".into(),
                    status: AgentCheckStatus::Fail,
                    message: "Managed package is incomplete; reinstall it".into(),
                });
            }
        } else if managed.is_err() {
            checks.push(AgentCatalogCheck {
                id: "managed_integrity".into(),
                status: AgentCheckStatus::Fail,
                message: "Managed installation record is invalid; inspect the dedicated directory"
                    .into(),
            });
        }
        if installed.is_none() {
            let path = self.lookup_agent(id, command, &tools, cancel).await?;
            if let (Some(path), Some((package, _))) = (&path, npm) {
                installed = package_beside_command(path, package, command).await;
                if installed.is_some() {
                    source = AgentInstallSource::System;
                }
            }
            let prefix = if npm.is_some() && installed.is_none() && path.is_none() {
                self.npm_prefix(&tools, cancel).await?
            } else {
                None
            };
            if let (Some(prefix), Some((package, _))) = (&prefix, npm) {
                // npm -g uses lib/node_modules on Unix and node_modules on Windows.
                let package_prefix = if cfg!(windows) {
                    prefix.clone()
                } else {
                    prefix.join("lib")
                };
                if let Ok(found) = paths::package_entry(&package_prefix, package, command).await {
                    installed = Some(found);
                    source = AgentInstallSource::System;
                }
            }
            if installed.is_none()
                && let Some(path) = path
            {
                installed = Some((String::new(), path));
                source = AgentInstallSource::System;
            }
        }
        let mut dependencies = vec![];
        let mut needs_node = if let Some((_, path)) = &installed {
            paths::requires_node(path).await?
        } else {
            npm.is_some()
        };
        for dependency in &entry.dependencies {
            let managed =
                if let (Some(prefix), Some(package)) = (&installed_prefix, &dependency.package) {
                    paths::package_entry(prefix, package, &dependency.command)
                        .await
                        .ok()
                } else {
                    None
                };
            let (path, actual) = if let Some((actual, path)) = managed {
                (Some(paths::command_path(&path)), Some(actual))
            } else if let Some(path) = tools.lookup(&dependency.command) {
                let actual = if let Ok(launch) = paths::launch(&path, &tools.node).await {
                    self.probe(&launch, &tools, cancel).await?
                } else {
                    None
                };
                (Some(path.to_string_lossy().into_owned()), actual)
            } else {
                (None, None)
            };
            if dependency.required
                && let Some(path) = &path
            {
                needs_node |= paths::requires_node(Path::new(path)).await?;
            }
            if dependency.required && path.is_none() {
                checks.push(AgentCatalogCheck {
                    id: format!("dependency_{}", dependency.command),
                    status: AgentCheckStatus::Fail,
                    message: dependency.instructions.clone(),
                });
            }
            dependencies.push(AgentDependencyInspection {
                command: dependency.command.clone(),
                required: dependency.required,
                path,
                version: actual,
            });
        }
        if needs_node {
            let required = npm.map(|(_, required)| required).unwrap_or("0.0.0");
            let actual = if tools.node.is_file() {
                self.probe(
                    &CommandSpec {
                        command: paths::command_path(&tools.node),
                        args: vec![],
                    },
                    &tools,
                    cancel,
                )
                .await?
            } else {
                None
            };
            checks.push(node_check(actual.as_deref(), required));
        }
        if npm.is_some() {
            checks.push(AgentCatalogCheck {
                id: "npm".into(),
                status: if tools.npm.is_some() { AgentCheckStatus::Pass } else if installed.is_some() { AgentCheckStatus::Warn } else { AgentCheckStatus::Fail },
                message: if tools.npm.is_some() { "npm command found" } else { "npm is unavailable; preparing managed connection components requires Node.js with npm" }.into(),
            });
        }
        let found_entry = installed.is_some();
        checks.push(AgentCatalogCheck {
            id: "entry".into(),
            status: if found_entry {
                AgentCheckStatus::Pass
            } else {
                AgentCheckStatus::Fail
            },
            message: if found_entry {
                "Agent entry point found; check the ACP connection before use"
            } else {
                "Agent entry point was not found; install it or specify its location"
            }
            .into(),
        });
        checks.push(AgentCatalogCheck {
            id: "managed_feedback".into(),
            status: if entry.verification.status == AgentVerificationStatus::Unsupported {
                AgentCheckStatus::Fail
            } else {
                AgentCheckStatus::Warn
            },
            message: entry.verification.note,
        });
        let mut launch = None;
        let mut actual = None;
        let dependency_commands = dependencies
            .iter()
            .filter(|dependency| dependency.required && installed_prefix.is_none())
            .filter_map(|dependency| dependency.path.as_ref().map(PathBuf::from))
            .collect::<Vec<_>>();
        let mut launch_env = runtime_environment(&tools.node, &dependency_commands);
        if let Some((version, path)) = installed {
            actual = (!version.is_empty()).then_some(version);
            match paths::launch(&path, &tools.node).await {
                Ok(found) => {
                    if actual.is_none() {
                        actual = self.probe(&found, &tools, cancel).await?;
                    }
                    launch = Some(found);
                },
                Err(_) => checks.push(AgentCatalogCheck { id: "launch".into(), status: AgentCheckStatus::Fail, message: "The entry point was found but cannot launch; check its runtime and executable permissions".into() }),
            }
        }
        if let Some(prefix) = &installed_prefix {
            launch_env.extend(managed_launch_environment(id, prefix));
        } else if id == "pi-acp"
            && let Some(path) = dependencies
                .iter()
                .find(|dependency| dependency.command == "pi")
                .and_then(|dependency| dependency.path.as_ref())
        {
            launch_env.insert("PI_ACP_PI_COMMAND".into(), path.clone());
            launch_env.insert("PI_ACP_ENABLE_EMBEDDED_CONTEXT".into(), "true".into());
        }
        Ok(AgentInspection {
            agent_id: id.into(),
            env: (!launch_env.is_empty()).then_some(launch_env),
            source,
            version: actual,
            command: launch.as_ref().map(|launch| launch.command.clone()),
            args: launch
                .map(|launch| launch.args.into_iter().chain(entry.args).collect())
                .unwrap_or_default(),
            dependencies,
            checks,
        })
    }
}

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod tests;
