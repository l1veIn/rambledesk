//! Explicit network gate, excluded from ordinary tests. Installs catalog-pinned
//! packages into a new app-owned temporary prefix, never globally. It only sends
//! ACP initialize and a DSH session: no authentication flow, prompt or model request.
//! Run: python scripts/onboarding-isolated-smoke.py (Windows clean process environment)
//! Or: cargo test -p rambledesk-acp --test catalog_network real_catalog_install_inspect_and_initialize -- --ignored --nocapture
use rambledesk_acp::{
    agents::AgentCatalogService,
    probe::{AcpConnection, AcpLaunch},
};
use rambledesk_core::{AgentCheckStatus, AgentInstallSource, InstallAgentInput};
use std::{collections::BTreeMap, sync::Arc};
use tokio_util::sync::CancellationToken;

#[tokio::test]
#[ignore = "requires the allowlisted environment from scripts/onboarding-isolated-smoke.py"]
async fn isolated_missing_runtime_reports_setup_requirements() {
    assert_eq!(
        std::env::var("RAMBLEDESK_ISOLATED_CATALOG_SMOKE").as_deref(),
        Ok("1")
    );
    assert!(rambledesk_core::find_executable("node").is_none());
    assert!(rambledesk_core::find_executable("npm").is_none());
    let root = tempfile::tempdir().unwrap();
    let service = AgentCatalogService::new(root.path().join("agents")).unwrap();
    let inspection = service
        .inspect_with_cancel("deepseek-acp", &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(inspection.source, AgentInstallSource::Missing);
    assert!(inspection.command.is_none());
    assert!(
        inspection
            .checks
            .iter()
            .any(|check| check.id == "npm" && check.status == AgentCheckStatus::Fail)
    );
    let result = service
        .install_with_cancel(
            InstallAgentInput {
                agent_id: "deepseek-acp".into(),
                version: None,
            },
            CancellationToken::new(),
            Arc::new(|_| {}),
        )
        .await;
    assert!(matches!(
        result,
        Err(rambledesk_acp::agents::CatalogError::CommandUnavailable)
    ));
}

#[tokio::test]
#[ignore = "downloads real npm packages into an isolated temporary prefix"]
async fn real_catalog_install_inspect_and_initialize() -> Result<(), Box<dyn std::error::Error>> {
    // Retain successful packages and a nonsecret report for reproduction. This
    // directory is created by the test; it cannot refer to user/global packages.
    let root = if let Some(path) = std::env::var_os("RAMBLEDESK_TEST_CATALOG_ROOT") {
        let path = std::path::PathBuf::from(path);
        assert!(path.is_absolute());
        assert!(
            path.canonicalize()?
                .starts_with(std::env::temp_dir().canonicalize()?)
        );
        assert!(
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("rambledesk-catalog-network-"))
        );
        assert!(path.join("agents/.rambledesk-agents").is_file());
        path
    } else {
        tempfile::Builder::new()
            .prefix("rambledesk-catalog-network-")
            .tempdir()?
            .keep()
    };
    println!("isolated evidence: {}", root.display());
    let service = AgentCatalogService::new(root.join("agents"))?;
    let mut report = vec![];
    for id in ["deepseek-acp", "codex-acp"] {
        if std::env::var("RAMBLEDESK_ISOLATED_CATALOG_SMOKE").as_deref() == Ok("1") {
            let before = service
                .inspect_with_cancel(id, &CancellationToken::new())
                .await?;
            assert_eq!(
                before.source,
                AgentInstallSource::Missing,
                "must not discover a preinstalled Agent"
            );
            assert!(before.command.is_none());
            assert!(
                before
                    .checks
                    .iter()
                    .any(|check| check.id == "node" && check.status == AgentCheckStatus::Pass)
            );
            assert!(
                before
                    .checks
                    .iter()
                    .any(|check| check.id == "npm" && check.status == AgentCheckStatus::Pass)
            );
        }
        let agent_name = id.to_owned();
        let installed = service
            .install_with_cancel(
                InstallAgentInput {
                    agent_id: id.into(),
                    version: None,
                },
                CancellationToken::new(),
                Arc::new(move |event| {
                    assert!(event.message.len() < 1024);
                    println!("{agent_name}: {:?}: {}", event.phase, event.message);
                }),
            )
            .await?;
        let inspection = service
            .inspect_with_cancel(id, &CancellationToken::new())
            .await?;
        assert_eq!(inspection.source, AgentInstallSource::Managed);
        assert_eq!(
            inspection.version.as_deref(),
            Some(installed.version.as_str())
        );
        assert_eq!(
            inspection.command.as_deref(),
            Some(installed.config.command.as_str())
        );
        assert!(!installed.config.args.is_empty());
        let entry = std::path::PathBuf::from(&installed.config.args[0]);
        assert!(entry.canonicalize()?.starts_with(root.canonicalize()?));
        let home = root.join(id).join("empty-home");
        tokio::fs::create_dir_all(&home).await?;
        let mut env = installed.config.env;
        for (name, path) in [
            ("HOME", home.clone()),
            ("USERPROFILE", home.clone()),
            ("APPDATA", home.join("roaming")),
            ("LOCALAPPDATA", home.join("local")),
            ("CODEX_HOME", home.join("codex")),
            ("XDG_CONFIG_HOME", home.join("config")),
            ("XDG_DATA_HOME", home.join("data")),
            ("XDG_CACHE_HOME", home.join("cache")),
            ("DSH_HOME", home.join("dsh")),
            ("DSH_AGENTS_HOME", home.join("dsh-agents")),
            ("DEEPSEEK_ACP_SESSIONS_ROOT", home.join("dsh-sessions")),
        ] {
            tokio::fs::create_dir_all(&path).await?;
            env.insert(name.into(), path.to_string_lossy().into_owned());
        }
        // Explicit inert placeholder: this probe never authenticates or sends a
        // model turn, even if the parent environment contains usable credentials.
        env.extend(BTreeMap::from([
            ("DEEPSEEK_API_KEY".into(), "unused-install-probe".into()),
            ("OPENAI_API_KEY".into(), "unused-install-probe".into()),
        ]));
        let launch = AcpLaunch {
            command: installed.config.command,
            args: installed.config.args,
            env,
            cwd: home,
            mcp_servers: vec![],
        };
        let connection = AcpConnection::connect(&launch, Arc::new(|_| {})).await?;
        let capabilities = connection.capabilities();
        let session_opened = if id == "deepseek-acp" {
            let session = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                connection.open_session(&launch, None),
            )
            .await??;
            assert!(!session.remote_session_id.is_empty());
            true
        } else {
            false
        };
        let shutdown = connection.shutdown().await;
        shutdown?;
        if id == "deepseek-acp" {
            assert!(capabilities.http_mcp);
            assert!(capabilities.load_session || capabilities.resume_session);
        }
        let item = serde_json::json!({
            "agent_id": id, "version": installed.version,
            "source": "managed", "entry_filename": entry.file_name().and_then(|name| name.to_str()),
            "entry_path": entry, "node_path": launch.command,
            "initialize": true, "capabilities": capabilities,
            "session_opened": session_opened, "authentication_verified": false,
            "shutdown": true, "model_requests": 0,
        });
        println!("{}", serde_json::to_string(&item)?);
        report.push(item);
        tokio::fs::write(
            root.join("report.json"),
            serde_json::to_vec_pretty(&report)?,
        )
        .await?;
    }
    Ok(())
}
