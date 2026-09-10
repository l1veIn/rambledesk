use super::*;
use std::time::Duration;

fn fixture() -> (tempfile::TempDir, AgentCatalogService) {
    let directory = tempfile::tempdir().unwrap();
    let mut service = AgentCatalogService::new(directory.path().join("managed")).unwrap();
    service.probe_timeout = Duration::from_secs(3);
    service.tools = Some(Toolchain {
        node: PathBuf::new(),
        npm: None,
        commands: Some(BTreeMap::new()),
        env: BTreeMap::new(),
    });
    (directory, service)
}

fn command(service: &mut AgentCatalogService, name: &str, path: &Path) {
    service
        .tools
        .as_mut()
        .unwrap()
        .commands
        .as_mut()
        .unwrap()
        .insert(name.into(), path.into());
}

fn node(service: &mut AgentCatalogService) -> PathBuf {
    let node = find_executable("node").expect("Node is required for subprocess fixtures");
    service.tools.as_mut().unwrap().node = node.clone();
    node
}

fn status<'a>(found: &'a AgentInspection, id: &str) -> Option<&'a AgentCheckStatus> {
    found
        .checks
        .iter()
        .find(|check| check.id == id)
        .map(|check| &check.status)
}

#[test]
fn fallback_runtime_path_preserves_existing_precedence_and_adds_each_directory_once() {
    let directory = tempfile::tempdir().unwrap();
    let node = directory.path().join("node.exe");
    let pi = directory.path().join("pi.cmd");
    std::fs::write(&node, "native fixture").unwrap();
    std::fs::write(&pi, "command fixture").unwrap();
    let original = std::env::var_os("PATH").unwrap_or_default();
    let expected_prefix = std::env::split_paths(&original).collect::<Vec<_>>();
    let env = runtime_environment(&node, &[pi]);
    assert_eq!(env.len(), 1);
    let actual = std::env::split_paths(&env["PATH"]).collect::<Vec<_>>();
    assert_eq!(&actual[..expected_prefix.len()], expected_prefix.as_slice());
    assert_eq!(actual.last().unwrap(), directory.path());
    assert_eq!(actual.len(), expected_prefix.len() + 1);
}

#[tokio::test]
async fn native_npm_distributed_agent_runs_without_node_or_npm() {
    let (_directory, mut service) = fixture();
    // A real native binary supplies a harmless --version fixture. Distribution
    // metadata must not impose Node.js on an already installed native entry.
    let native = find_executable("node").unwrap();
    command(&mut service, "grok", &native);
    let found = service
        .inspect_inner("grok", &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(found.source, AgentInstallSource::System);
    assert_eq!(status(&found, "node"), None);
    assert_eq!(status(&found, "npm"), Some(&AgentCheckStatus::Warn));
    assert!(found.command.is_some());
    assert!(
        found
            .checks
            .iter()
            .all(|check| check.status != AgentCheckStatus::Fail)
    );
    assert!(
        service
            .install_inner(
                InstallAgentInput {
                    agent_id: "grok".into(),
                    version: None
                },
                &CancellationToken::new(),
                std::sync::Arc::new(|_| {})
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn javascript_without_node_reports_runtime_failure_instead_of_losing_discovery() {
    let (directory, mut service) = fixture();
    let path = directory.path().join("agent.mjs");
    tokio::fs::write(&path, "console.log('deepseek-acp 1.0.0');")
        .await
        .unwrap();
    command(&mut service, "deepseek-acp", &path);
    let found = service
        .inspect_inner("deepseek-acp", &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(found.source, AgentInstallSource::System);
    assert_eq!(status(&found, "entry"), Some(&AgentCheckStatus::Pass));
    assert_eq!(status(&found, "node"), Some(&AgentCheckStatus::Fail));
    assert!(found.command.is_none());
    assert_eq!(
        node_check(Some("18.20.0"), "22.0.0").status,
        AgentCheckStatus::Fail
    );
    assert_eq!(
        node_check(Some("22.22.3"), "22.0.0").status,
        AgentCheckStatus::Pass
    );
}

#[tokio::test]
async fn javascript_with_old_node_is_blocked_by_the_version_check() {
    let (directory, mut service) = fixture();
    let old_node = directory
        .path()
        .join(if cfg!(windows) { "node.cmd" } else { "node" });
    tokio::fs::write(
        &old_node,
        if cfg!(windows) {
            "@echo off\r\necho v18.20.0\r\n"
        } else {
            "#!/bin/sh\necho v18.20.0\n"
        },
    )
    .await
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&old_node, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    service.tools.as_mut().unwrap().node = old_node;
    let path = directory.path().join("agent.mjs");
    tokio::fs::write(&path, "console.log('agent 1.0.0');")
        .await
        .unwrap();
    command(&mut service, "deepseek-acp", &path);
    let found = service
        .inspect_inner("deepseek-acp", &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(status(&found, "node"), Some(&AgentCheckStatus::Fail));
    assert!(
        found
            .checks
            .iter()
            .find(|check| check.id == "node")
            .unwrap()
            .message
            .contains("18.20.0")
    );
}

#[tokio::test]
async fn cursor_alias_requires_identity_and_keeps_the_specific_command_first() {
    let (directory, mut service) = fixture();
    node(&mut service);
    let alias = directory.path().join("agent.mjs");
    tokio::fs::write(&alias, "console.log('Unrelated agent utility 1.0.0');")
        .await
        .unwrap();
    command(&mut service, "agent", &alias);
    let cancel = CancellationToken::new();
    assert!(
        service
            .lookup_agent("cursor", "cursor-agent", &service.tools().await, &cancel)
            .await
            .unwrap()
            .is_none()
    );
    tokio::fs::write(&alias, "console.log('Cursor Agent CLI 1.0.0');")
        .await
        .unwrap();
    let found = service.inspect_inner("cursor", &cancel).await.unwrap();
    assert_eq!(found.source, AgentInstallSource::System);
    assert!(found.args.ends_with(&["acp".into()]));
    let primary = directory.path().join("cursor-agent.mjs");
    tokio::fs::write(&primary, "console.log('Cursor Agent 2.0.0');")
        .await
        .unwrap();
    command(&mut service, "cursor-agent", &primary);
    assert_eq!(
        service
            .lookup_agent("cursor", "cursor-agent", &service.tools().await, &cancel)
            .await
            .unwrap(),
        Some(primary)
    );
    assert!(
        service
            .lookup_agent("hermes", "hermes", &service.tools().await, &cancel)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn managed_javascript_missing_runtime_is_not_reported_as_corrupt() {
    let (directory, service) = fixture();
    let root = paths::prepare_root(&service.root).await.unwrap();
    let generation = uuid::Uuid::now_v7().to_string();
    let prefix = root.join("deepseek-acp/versions").join(&generation);
    let package = prefix.join("node_modules/deepseek-acp");
    tokio::fs::create_dir_all(&package).await.unwrap();
    tokio::fs::write(
        package.join("package.json"),
        r#"{"name":"deepseek-acp","version":"0.8.0","bin":{"deepseek-acp":"entry.mjs"}}"#,
    )
    .await
    .unwrap();
    tokio::fs::write(package.join("entry.mjs"), "console.log('0.8.0');")
        .await
        .unwrap();
    tokio::fs::write(
        root.join("deepseek-acp/current.json"),
        serde_json::to_vec(&paths::Current { generation }).unwrap(),
    )
    .await
    .unwrap();
    let found = service
        .inspect_inner("deepseek-acp", &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(found.source, AgentInstallSource::Managed);
    assert_eq!(found.version.as_deref(), Some("0.8.0"));
    assert_eq!(status(&found, "node"), Some(&AgentCheckStatus::Fail));
    assert_eq!(status(&found, "managed_integrity"), None);
    assert!(directory.path().exists());
}

#[tokio::test]
async fn system_pi_dependency_is_required_and_its_discovered_path_is_reused() {
    let (directory, mut service) = fixture();
    node(&mut service);
    let bridge = directory.path().join("bridge.mjs");
    let pi = directory.path().join("pi.mjs");
    for path in [&bridge, &pi] {
        tokio::fs::write(path, "console.log('pi 0.83.0');")
            .await
            .unwrap();
    }
    command(&mut service, "pi-acp", &bridge);
    let missing = service
        .inspect_inner("pi-acp", &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        status(&missing, "dependency_pi"),
        Some(&AgentCheckStatus::Fail)
    );
    command(&mut service, "pi", &pi);
    let found = service
        .inspect_inner("pi-acp", &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(status(&found, "dependency_pi"), None);
    assert_eq!(
        found.env.as_ref().unwrap()["PI_ACP_PI_COMMAND"],
        pi.to_string_lossy()
    );
}

#[tokio::test]
async fn npm_shim_can_resolve_a_native_package_without_npm_or_node() {
    let (directory, mut service) = fixture();
    let package = directory.path().join("node_modules/@xai-official/grok");
    tokio::fs::create_dir_all(&package).await.unwrap();
    let native = package.join(if cfg!(windows) { "grok.exe" } else { "grok" });
    tokio::fs::copy(find_executable("node").unwrap(), &native)
        .await
        .unwrap();
    tokio::fs::write(
        package.join("package.json"),
        format!(
            r#"{{"name":"@xai-official/grok","version":"1.0.13","bin":{{"grok":"{}"}}}}"#,
            native.file_name().unwrap().to_string_lossy()
        ),
    )
    .await
    .unwrap();
    let shim = directory.path().join("grok.cmd");
    tokio::fs::write(&shim, "@echo off\r\n").await.unwrap();
    command(&mut service, "grok", &shim);
    let found = service
        .inspect_inner("grok", &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(found.version.as_deref(), Some("1.0.13"));
    assert_eq!(status(&found, "node"), None);
    assert_eq!(
        found.command,
        Some(paths::command_path(
            &tokio::fs::canonicalize(native).await.unwrap()
        ))
    );
}
