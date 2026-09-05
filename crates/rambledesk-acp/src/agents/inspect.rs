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
            BTreeMap::new()
        }
    }
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

impl AgentCatalogService {
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
        if let Some((_, required)) = npm {
            let node = if tools.node.is_file() {
                self.probe(
                    &CommandSpec {
                        command: tools.node.to_string_lossy().into_owned(),
                        args: vec![],
                    },
                    &tools,
                    cancel,
                )
                .await?
            } else {
                None
            };
            checks.push(AgentCatalogCheck {
                id: "node".into(),
                status: if node
                    .as_deref()
                    .is_some_and(|node| version::meets(node, required))
                {
                    AgentCheckStatus::Pass
                } else {
                    AgentCheckStatus::Fail
                },
                message: format!(
                    "Node.js {} (requires >= {required})",
                    node.as_deref().unwrap_or("unavailable or version unknown")
                ),
            });
            checks.push(AgentCatalogCheck {
                id: "npm".into(),
                status: if tools.npm.is_some() {
                    AgentCheckStatus::Pass
                } else {
                    AgentCheckStatus::Fail
                },
                message: if tools.npm.is_some() {
                    "npm command found"
                } else {
                    "Install Node.js with npm to install managed packages"
                }
                .into(),
            });
        }
        let managed = paths::current(&self.root, id).await;
        let mut installed = None;
        let mut source = AgentInstallSource::Missing;
        let mut installed_prefix = None;
        if let (Ok(Some(prefix)), Some((package, _))) = (&managed, npm) {
            if let Ok(found) = paths::package(prefix, package, command, &tools.node).await {
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
            let path = tools.lookup(command);
            let prefix = if npm.is_some() {
                self.npm_prefix(&tools, cancel).await?
            } else {
                None
            };
            if let (Some(prefix), Some((package, _))) = (&prefix, npm) {
                let bin_dir = if cfg!(windows) {
                    prefix.clone()
                } else {
                    prefix.join("bin")
                };
                if path
                    .as_ref()
                    .is_none_or(|path| path.parent() == Some(bin_dir.as_path()))
                {
                    // npm -g uses lib/node_modules on Unix and node_modules on Windows.
                    let package_prefix = if cfg!(windows) {
                        prefix.clone()
                    } else {
                        prefix.join("lib")
                    };
                    if let Ok(found) =
                        paths::package(&package_prefix, package, command, &tools.node).await
                    {
                        installed = Some(found);
                        source = AgentInstallSource::System;
                    }
                }
            }
            if installed.is_none()
                && let Some(path) = path
            {
                let launch = paths::launch(&path, &tools.node).await?;
                let actual = self.probe(&launch, &tools, cancel).await?;
                installed = Some((actual.unwrap_or_default(), launch));
                source = AgentInstallSource::System;
            }
        }
        let mut dependencies = vec![];
        if installed.is_some()
            && let Some(check) = checks
                .iter_mut()
                .find(|check| check.id == "npm" && check.status == AgentCheckStatus::Fail)
        {
            check.status = AgentCheckStatus::Warn;
            check.message = "npm is unavailable; the installed Agent can run, but managed installation and updates require npm".into();
        }
        for dependency in &entry.dependencies {
            let managed =
                if let (Some(prefix), Some(package)) = (&installed_prefix, &dependency.package) {
                    paths::package(prefix, package, &dependency.command, &tools.node)
                        .await
                        .ok()
                } else {
                    None
                };
            let (path, actual) = if let Some((actual, launch)) = managed {
                (
                    Some(launch.args.first().cloned().unwrap_or(launch.command)),
                    Some(actual),
                )
            } else if let Some(path) = tools.lookup(&dependency.command) {
                let launch = paths::launch(&path, &tools.node).await?;
                let actual = self.probe(&launch, &tools, cancel).await?;
                (Some(path.to_string_lossy().into_owned()), actual)
            } else {
                (None, None)
            };
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
        checks.push(AgentCatalogCheck { id: "entry".into(), status: if installed.is_some() { AgentCheckStatus::Pass } else { AgentCheckStatus::Fail }, message: if installed.is_some() { "Agent entry point found; authentication and ACP capabilities still require a connection check" } else { "Agent entry point was not found" }.into() });
        checks.push(AgentCatalogCheck {
            id: "managed_feedback".into(),
            status: if entry.verification.status == AgentVerificationStatus::Unsupported {
                AgentCheckStatus::Fail
            } else {
                AgentCheckStatus::Warn
            },
            message: entry.verification.note,
        });
        let (actual, launch) = installed
            .map(|(actual, launch)| {
                (
                    if actual.is_empty() {
                        None
                    } else {
                        Some(actual)
                    },
                    Some(launch),
                )
            })
            .unwrap_or((None, None));
        Ok(AgentInspection {
            agent_id: id.into(),
            env: installed_prefix
                .as_ref()
                .map(|prefix| managed_launch_environment(id, prefix))
                .filter(|env| !env.is_empty()),
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
