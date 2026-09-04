// Adapted from Codeg 3ebdfed commands/acp.rs npm installation and
// acp/binary_cache.rs staged publication (Apache-2.0). Changed: never install
// globally or retry with --force; immutable generations retain working installs,
// metadata/bin verification precedes atomic publication, and cancellation cleans
// only this operation's owned process tree and staging generation.
use super::{
    AgentCatalogService, CatalogError, catalog, paths,
    runner::{self, CommandSpec},
    version,
};
use rambledesk_core::*;
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub(super) struct Registration {
    active: Arc<Mutex<HashMap<String, CancellationToken>>>,
    id: String,
    removed: bool,
}
impl Registration {
    pub fn new(active: Arc<Mutex<HashMap<String, CancellationToken>>>, id: String) -> Self {
        Self {
            active,
            id,
            removed: false,
        }
    }
    pub async fn remove(mut self) {
        self.active.lock().await.remove(&self.id);
        self.removed = true;
    }
}
impl Drop for Registration {
    fn drop(&mut self) {
        if self.removed {
            return;
        }
        if let Ok(mut active) = self.active.try_lock() {
            active.remove(&self.id);
        } else if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let active = self.active.clone();
            let id = self.id.clone();
            runtime.spawn(async move {
                active.lock().await.remove(&id);
            });
        }
    }
}

fn progress(observer: &AgentInstallObserver, phase: AgentInstallPhase, message: impl Into<String>) {
    observer(AgentInstallProgress {
        phase,
        message: message.into(),
    });
}

impl AgentCatalogService {
    pub(super) async fn install_inner(
        &self,
        input: InstallAgentInput,
        cancel: &CancellationToken,
        observer: AgentInstallObserver,
    ) -> Result<InstalledAgent, CatalogError> {
        let entry = catalog::entry(&input.agent_id).map_err(|_| CatalogError::UnknownAgent)?;
        let AgentDistribution::Npm {
            package,
            pinned_version,
            command,
            node_required,
        } = &entry.distribution
        else {
            return Err(CatalogError::ManualInstall);
        };
        let requested = input
            .version
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(pinned_version);
        let requested = version::sanitize(requested).ok_or(CatalogError::InvalidVersion)?;
        if cancel.is_cancelled() {
            return Err(CatalogError::Cancelled);
        }
        progress(
            &observer,
            AgentInstallPhase::Preparing,
            format!("Preparing {} {requested}", entry.name),
        );
        let tools = self.tools().await;
        let npm = tools.npm.as_ref().ok_or(CatalogError::CommandUnavailable)?;
        let node = CommandSpec {
            command: tools.node.to_string_lossy().into_owned(),
            args: vec![],
        };
        let actual = self
            .probe(&node, &tools, cancel)
            .await?
            .ok_or(CatalogError::CommandUnavailable)?;
        if !version::meets(&actual, node_required) {
            return Err(CatalogError::NodeVersion);
        }
        let root = {
            // Different Agent packages may install concurrently, but the shared
            // directory marker must be initialized before another job reads it.
            let _root = self.root_gate.lock().await;
            paths::prepare_root(&self.root).await?
        };
        let agent_root = paths::directory(&root, &root.join(&entry.id)).await?;
        let generation = uuid::Uuid::now_v7().to_string();
        let mut staging = paths::Staging {
            root: root.clone(),
            path: agent_root.join(format!(".staging-{generation}")),
            published: false,
        };
        let result = async {
            paths::directory(&root, &staging.path).await?;
            let cache = paths::directory(&root, &root.join("npm-cache")).await?;
            let mut args = vec![
                "install".into(),
                "--global=false".into(),
                "--prefix".into(),
                paths::command_path(&staging.path),
                "--cache".into(),
                paths::command_path(&cache),
                "--include=optional".into(),
                "--foreground-scripts".into(),
                "--ignore-scripts=false".into(),
                "--no-audit".into(),
                "--no-fund".into(),
                "--package-lock=false".into(),
                "--save-exact".into(),
                "--registry=https://registry.npmjs.org".into(),
                format!("{package}@{requested}"),
            ];
            for dependency in entry
                .dependencies
                .iter()
                .filter(|dependency| dependency.required)
            {
                if let (Some(package), Some(version)) =
                    (&dependency.package, &dependency.pinned_version)
                {
                    args.push(format!("{package}@{version}"));
                }
            }
            progress(
                &observer,
                AgentInstallPhase::Installing,
                "Installing packages into RambleDesk's dedicated directory",
            );
            runner::run(
                npm,
                &args,
                &staging.path,
                &tools.env(),
                self.install_timeout,
                cancel,
            )
            .await?;
            progress(
                &observer,
                AgentInstallPhase::Verifying,
                "Checking installed package versions and entry points",
            );
            let (installed, _) =
                paths::package(&staging.path, package, command, &tools.node).await?;
            if installed != requested {
                return Err(CatalogError::InvalidInstall);
            }
            for dependency in entry
                .dependencies
                .iter()
                .filter(|dependency| dependency.required)
            {
                if let (Some(package), Some(version)) =
                    (&dependency.package, &dependency.pinned_version)
                {
                    let (actual, _) =
                        paths::package(&staging.path, package, &dependency.command, &tools.node)
                            .await?;
                    if &actual != version {
                        return Err(CatalogError::InvalidInstall);
                    }
                }
            }
            if cancel.is_cancelled() {
                return Err(CatalogError::Cancelled);
            }
            let versions = paths::directory(&root, &agent_root.join("versions")).await?;
            let destination = versions.join(&generation);
            // Keep the rename and guard update in one poll so dropping this
            // future cannot strand a completed filesystem operation.
            std::fs::rename(&staging.path, &destination).map_err(|_| CatalogError::Storage)?;
            staging.path = destination;
            let (_, launch) = paths::package(&staging.path, package, command, &tools.node).await?;
            let mut env = BTreeMap::new();
            if entry.id == "pi-acp" {
                env.insert("PI_ACP_ENABLE_EMBEDDED_CONTEXT".into(), "true".into());
                env.insert(
                    "PI_ACP_PI_COMMAND".into(),
                    paths::command_path(&staging.path.join(if cfg!(windows) {
                        "node_modules/.bin/pi.cmd"
                    } else {
                        "node_modules/.bin/pi"
                    })),
                );
            }
            let config = SaveAgentConfigInput {
                id: None,
                name: entry.name.clone(),
                host_id: entry.host_id.clone(),
                protocol: SessionProtocol::Acp,
                enabled: entry.verification.status != AgentVerificationStatus::Unsupported,
                command: launch.command,
                args: launch.args.into_iter().chain(entry.args.clone()).collect(),
                env,
            };
            let pointer = staging.path.join(".current.json");
            tokio::fs::write(
                &pointer,
                serde_json::to_vec(&paths::Current {
                    generation: generation.clone(),
                })
                .map_err(|_| CatalogError::Storage)?,
            )
            .await
            .map_err(|_| CatalogError::Storage)?;
            if cancel.is_cancelled() {
                return Err(CatalogError::Cancelled);
            }
            // Commit point: earlier configs retain immutable old paths. After this
            // atomic replacement cancellation cannot turn a success into failure.
            std::fs::rename(&pointer, agent_root.join("current.json"))
                .map_err(|_| CatalogError::Storage)?;
            staging.published = true;
            progress(
                &observer,
                AgentInstallPhase::Complete,
                format!(
                    "Installed {} {installed}; run its connection check before use",
                    entry.name
                ),
            );
            Ok(InstalledAgent {
                agent_id: entry.id.clone(),
                version: installed,
                config,
            })
        }
        .await;
        if result.is_err() {
            staging.clean().await?;
        }
        result
    }
}
