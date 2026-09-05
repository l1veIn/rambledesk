use super::*;
use runner::CommandSpec;
use std::{collections::BTreeMap, sync::Mutex as StdMutex};

fn fixture(mode: &str) -> (tempfile::TempDir, AgentCatalogService) {
    let dir = tempfile::tempdir().unwrap();
    let node =
        rambledesk_core::find_executable("node").expect("Node is required for subprocess fixtures");
    let mut service = AgentCatalogService::new(dir.path().join("managed-agents")).unwrap();
    service.probe_timeout = Duration::from_secs(3);
    service.install_timeout = Duration::from_secs(5);
    service.tools = Some(inspect::Toolchain {
        node: node.clone(),
        npm: Some(CommandSpec {
            command: node.to_string_lossy().into_owned(),
            args: vec![
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("src/agents/fixture.mjs")
                    .to_string_lossy()
                    .into_owned(),
            ],
        }),
        commands: Some(BTreeMap::new()),
        env: BTreeMap::from([
            ("FIXTURE_MODE".into(), mode.into()),
            (
                "FIXTURE_GLOBAL_PREFIX".into(),
                dir.path().join("global").to_string_lossy().into_owned(),
            ),
            (
                "FIXTURE_STARTED".into(),
                dir.path().join("started").to_string_lossy().into_owned(),
            ),
            (
                "FIXTURE_HEARTBEAT".into(),
                dir.path().join("heartbeat").to_string_lossy().into_owned(),
            ),
        ]),
    });
    (dir, service)
}
fn input(id: &str, version: Option<&str>) -> InstallAgentInput {
    InstallAgentInput {
        agent_id: id.into(),
        version: version.map(String::from),
    }
}
fn quiet() -> AgentInstallObserver {
    Arc::new(|_| {})
}
async fn install(service: &AgentCatalogService, id: &str, version: Option<&str>) -> InstalledAgent {
    service
        .install_with_cancel(input(id, version), CancellationToken::new(), quiet())
        .await
        .unwrap()
}
async fn no_staging(service: &AgentCatalogService, id: &str) {
    let path = service.root.join(id);
    if !path.exists() {
        return;
    }
    let mut entries = tokio::fs::read_dir(path).await.unwrap();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        assert!(!entry.file_name().to_string_lossy().starts_with(".staging-"));
    }
}

#[tokio::test]
async fn five_npm_paths_install_detect_actual_versions_and_generate_launchable_configs() {
    let (_dir, service) = fixture("success");
    for id in ["deepseek-acp", "dsh", "codex-acp", "claude-acp", "pi-acp"] {
        let installed = install(&service, id, None).await;
        let found = service
            .inspect_with_cancel(id, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(found.source, AgentInstallSource::Managed);
        assert_eq!(found.env.clone().unwrap_or_default(), installed.config.env);
        assert_eq!(found.version.as_deref(), Some(installed.version.as_str()));
        assert_eq!(
            found.command.as_deref(),
            Some(installed.config.command.as_str())
        );
        let output = runner::run(
            &CommandSpec {
                command: installed.config.command.clone(),
                args: vec![installed.config.args[0].clone()],
            },
            &["--version".into()],
            &service.root,
            &BTreeMap::new(),
            Duration::from_secs(3),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert!(output.stdout.contains(&installed.version));
        let prefix = paths::current(&service.root, id).await.unwrap().unwrap();
        let args = paths::json(&prefix.join("arguments.json")).await.unwrap();
        assert!(
            args.as_array()
                .unwrap()
                .contains(&serde_json::json!("--global=false"))
        );
        assert!(
            !args
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("--force"))
        );
        if id == "dsh" {
            assert!(
                installed
                    .config
                    .args
                    .ends_with(&["--profile".into(), "acp".into()])
            );
        }
        if id == "pi-acp" {
            assert!(installed.config.enabled);
            assert_eq!(found.dependencies[0].version.as_deref(), Some("0.83.0"));
            assert!(std::path::Path::new(&installed.config.env["PI_ACP_PI_COMMAND"]).is_file());
            assert!(
                crate::pi_wrapper::is_pi_acp_recipe(
                    &installed.config.command,
                    &installed.config.args
                )
                .await
            );
            let native = crate::pi_wrapper::resolve_native_pi_for_agent(
                &installed.config.command,
                &installed.config.args,
                None,
            )
            .await
            .unwrap();
            assert!(native.args[0].contains("pi-coding-agent"));
            let explicit = crate::pi_wrapper::resolve_native_pi_for_agent(
                &installed.config.command,
                &installed.config.args,
                Some(&installed.config.env["PI_ACP_PI_COMMAND"]),
            )
            .await
            .unwrap();
            assert_eq!(explicit.command, native.command);
            assert_eq!(explicit.args, native.args);
            let other_script = prefix.join("node_modules/pi-acp/nested/other.mjs");
            tokio::fs::write(&other_script, "// not the package bin")
                .await
                .unwrap();
            assert!(
                !crate::pi_wrapper::is_pi_acp_recipe(
                    &installed.config.command,
                    &[other_script.to_string_lossy().into_owned()]
                )
                .await
            );
            let fake = prefix.join("pi-acp.exe");
            tokio::fs::write(&fake, "unrelated command").await.unwrap();
            assert!(!crate::pi_wrapper::is_pi_acp_recipe(&fake.to_string_lossy(), &[]).await);
            #[cfg(windows)]
            {
                let shim = prefix.join("node_modules/.bin/pi-acp.cmd");
                assert!(crate::pi_wrapper::is_pi_acp_recipe(&shim.to_string_lossy(), &[]).await);
                let through_shim = crate::pi_wrapper::resolve_native_pi_for_agent(
                    &shim.to_string_lossy(),
                    &[],
                    None,
                )
                .await
                .unwrap();
                assert_eq!(through_shim.args, native.args);
            }
        }
        no_staging(&service, id).await;
    }
}

#[tokio::test]
async fn historical_pi_defaults_follow_the_selected_generation_without_overwriting_user_env() {
    let (_dir, service) = fixture("success");
    let old = install(&service, "pi-acp", None).await.config;
    let expected = old.env.clone();
    let current = install(&service, "pi-acp", None).await.config;
    assert_ne!(old.args, current.args);
    let mut config = AgentConfig {
        id: "old".into(),
        catalog_id: old.catalog_id,
        name: old.name,
        host_id: old.host_id,
        protocol: old.protocol,
        enabled: old.enabled,
        command: old.command,
        args: old.args,
        env: BTreeMap::new(),
        created_at: "old".into(),
        updated_at: "old".into(),
    };
    let mut env = BTreeMap::new();
    apply_managed_pi_defaults(&config, &mut env).await;
    assert_eq!(env, expected);
    env.insert("PI_ACP_PI_COMMAND".into(), "explicit-native-pi".into());
    env.insert("CUSTOM".into(), "retain".into());
    apply_managed_pi_defaults(&config, &mut env).await;
    assert_eq!(env["PI_ACP_PI_COMMAND"], "explicit-native-pi");
    assert_eq!(env["CUSTOM"], "retain");
    config.catalog_id = None;
    let mut unrelated = BTreeMap::new();
    apply_managed_pi_defaults(&config, &mut unrelated).await;
    assert!(unrelated.is_empty());
    config.catalog_id = Some("pi-acp".into());
    tokio::fs::remove_file(service.root.join(".rambledesk-agents"))
        .await
        .unwrap();
    apply_managed_pi_defaults(&config, &mut unrelated).await;
    assert!(unrelated.is_empty());
}

#[tokio::test]
async fn failed_or_incomplete_updates_preserve_the_previous_generation_and_do_not_leak_logs() {
    let (_dir, mut service) = fixture("success");
    let original = install(&service, "deepseek-acp", None).await;
    for mode in ["fail", "wrong-version", "missing-bin", "escape-bin"] {
        service
            .tools
            .as_mut()
            .unwrap()
            .env
            .insert("FIXTURE_MODE".into(), mode.into());
        let events = Arc::new(StdMutex::new(Vec::new()));
        let sink = events.clone();
        let result = service
            .install_with_cancel(
                input("deepseek-acp", Some("0.9.0")),
                CancellationToken::new(),
                Arc::new(move |event| sink.lock().unwrap().push(event)),
            )
            .await;
        assert!(result.is_err());
        assert!(!format!("{}", result.unwrap_err()).contains("fixture-secret"));
        assert!(
            events.lock().unwrap().iter().all(
                |event| event.message.len() < 1024 && !event.message.contains("fixture-secret")
            )
        );
        let current = service
            .inspect_with_cancel("deepseek-acp", &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(current.version.as_deref(), Some(original.version.as_str()));
        no_staging(&service, "deepseek-acp").await;
    }
    service
        .tools
        .as_mut()
        .unwrap()
        .env
        .insert("FIXTURE_MODE".into(), "success".into());
    let updated = install(&service, "deepseek-acp", Some("0.9.0")).await;
    assert_ne!(original.config.args[0], updated.config.args[0]);
    assert!(std::path::Path::new(&original.config.args[0]).is_file());
    assert_eq!(
        service
            .inspect_with_cancel("deepseek-acp", &CancellationToken::new())
            .await
            .unwrap()
            .version
            .as_deref(),
        Some("0.9.0")
    );
}

#[tokio::test]
async fn cancelling_an_install_stops_its_real_child_tree_and_cleans_incomplete_files() {
    let (dir, service) = fixture("hang");
    let installing = service.clone();
    let task = tokio::spawn(async move {
        installing
            .install_with_cancel(
                input("deepseek-acp", None),
                CancellationToken::new(),
                quiet(),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(3), async {
        while !dir.path().join("heartbeat").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    AgentCatalogProvider::cancel_install(&service, "deepseek-acp")
        .await
        .unwrap();
    assert!(matches!(task.await.unwrap(), Err(CatalogError::Cancelled)));
    let heartbeat = tokio::fs::read(dir.path().join("heartbeat")).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        heartbeat,
        tokio::fs::read(dir.path().join("heartbeat")).await.unwrap()
    );
    assert!(
        paths::current(&service.root, "deepseek-acp")
            .await
            .unwrap()
            .is_none()
    );
    no_staging(&service, "deepseek-acp").await;
}

#[tokio::test]
async fn probes_are_bounded_and_manual_or_invalid_requests_do_not_install() {
    let (_dir, service) = fixture("success");
    for (id, version) in [
        ("hermes", None),
        ("cursor", None),
        ("deepseek-acp", Some("latest")),
        ("../../other", None),
    ] {
        assert!(
            service
                .install_with_cancel(input(id, version), CancellationToken::new(), quiet())
                .await
                .is_err()
        );
    }
    assert!(!service.root.exists());
    let tools = service.tools.as_ref().unwrap();
    let output = runner::run(
        tools.npm.as_ref().unwrap(),
        &["flood".into()],
        &std::env::temp_dir(),
        &BTreeMap::new(),
        Duration::from_secs(3),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(output.stdout.len(), 32 * 1024);
    assert_eq!(output.stderr.len(), 32 * 1024);
    assert_eq!(service.catalog().len(), 16);
}

#[tokio::test]
async fn existing_system_package_and_standalone_command_report_actual_not_catalog_versions() {
    let (dir, mut service) = fixture("success");
    let global = dir.path().join("global");
    let package_prefix = if cfg!(windows) {
        global.clone()
    } else {
        global.join("lib")
    };
    let tools = service.tools.as_ref().unwrap();
    runner::run(
        tools.npm.as_ref().unwrap(),
        &[
            "install".into(),
            "--prefix".into(),
            paths::command_path(&package_prefix),
            "deepseek-acp@0.6.0".into(),
        ],
        dir.path(),
        &tools.env(),
        Duration::from_secs(3),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let found = service
        .inspect_with_cancel("deepseek-acp", &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(found.source, AgentInstallSource::System);
    assert_eq!(found.version.as_deref(), Some("0.6.0"));
    assert!(!service.root.exists());

    // A PATH entry outside npm's prefix wins over a different globally installed
    // package. Its own --version output supplies evidence instead of a pin.
    let standalone = dir.path().join("standalone.mjs");
    tokio::fs::write(
        &standalone,
        "process.stderr.write('standalone-agent/0.5.1\\n');",
    )
    .await
    .unwrap();
    service
        .tools
        .as_mut()
        .unwrap()
        .commands
        .as_mut()
        .unwrap()
        .insert("deepseek-acp".into(), standalone.clone());
    let found = service
        .inspect_with_cancel("deepseek-acp", &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(found.source, AgentInstallSource::System);
    assert_eq!(found.version.as_deref(), Some("0.5.1"));
    assert_eq!(
        found.args[0],
        paths::command_path(&tokio::fs::canonicalize(&standalone).await.unwrap())
    );
    tokio::fs::write(&standalone, "process.stdout.write('version unavailable');")
        .await
        .unwrap();
    service.tools.as_mut().unwrap().npm = None;
    let found = service
        .inspect_with_cancel("deepseek-acp", &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(found.version, None);
    assert!(found.command.is_some());
    assert_eq!(
        found
            .checks
            .iter()
            .find(|check| check.id == "npm")
            .unwrap()
            .status,
        AgentCheckStatus::Warn
    );
    assert!(
        !found
            .checks
            .iter()
            .any(|check| check.status == AgentCheckStatus::Fail)
    );
}

#[tokio::test]
async fn timeouts_and_dropped_install_futures_release_processes_staging_and_registration() {
    let (dir, mut service) = fixture("hang");
    service.install_timeout = Duration::from_millis(250);
    assert!(matches!(
        service
            .install_with_cancel(
                input("deepseek-acp", None),
                CancellationToken::new(),
                quiet()
            )
            .await,
        Err(CatalogError::Timeout)
    ));
    no_staging(&service, "deepseek-acp").await;
    assert!(service.active.lock().await.is_empty());

    // Dropping the caller's future is a separate cleanup path from explicit
    // cancellation. Observe an owned descendant to prove it cannot keep running.
    service.install_timeout = Duration::from_secs(5);
    let heartbeat = dir.path().join("heartbeat");
    let previous = tokio::fs::read(&heartbeat).await.ok();
    let installing = service.clone();
    let task = tokio::spawn(async move {
        installing
            .install_with_cancel(
                input("deepseek-acp", None),
                CancellationToken::new(),
                quiet(),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(3), async {
        while tokio::fs::read(&heartbeat).await.ok() == previous {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let mut entries = tokio::fs::read_dir(service.root.join("deepseek-acp"))
                .await
                .unwrap();
            let mut pending = false;
            while let Some(entry) = entries.next_entry().await.unwrap() {
                pending |= entry.file_name().to_string_lossy().starts_with(".staging-");
            }
            if !pending && service.active.lock().await.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let value = tokio::fs::read(&heartbeat).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(value, tokio::fs::read(&heartbeat).await.unwrap());
    assert!(
        paths::current(&service.root, "deepseek-acp")
            .await
            .unwrap()
            .is_none()
    );
}
