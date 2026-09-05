use std::collections::BTreeMap;

use rambledesk_core::{
    AgentConfig, NewManagedSession, SessionManagement, SessionProtocol, SessionRepository,
    SessionRepositoryError,
};
use sqlx::migrate::Migrate;

use super::*;

#[path = "managed_sessions/prepared.rs"]
mod prepared;

const CREATED: &str = "2026-09-04T00:00:00Z";
const UPDATED: &str = "2026-09-04T01:00:00Z";

#[tokio::test]
async fn catalog_identity_migration_links_only_unambiguous_historical_recipes_and_preserves_settings()
 {
    let workspace = TestWorkspace::new().await;
    tokio::fs::create_dir_all(workspace.database.parent().unwrap())
        .await
        .unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&workspace.database)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    let mut connection = pool.acquire().await.unwrap();
    connection.ensure_migrations_table().await.unwrap();
    for migration in MIGRATOR.iter().filter(|migration| migration.version <= 17) {
        connection.apply(migration).await.unwrap();
    }
    let cases = [
        (
            "legacy-command",
            "dsh",
            r"C:\Tools\deepseek-acp.CMD",
            r#"["--custom","literal value"]"#,
            Some("deepseek-acp"),
        ),
        (
            "legacy-package",
            "pi",
            "node",
            r#"["C:\\old\\node_modules\\pi-acp\\dist\\index.js","--custom"]"#,
            Some("pi-acp"),
        ),
        (
            "ambiguous",
            "dsh",
            "node",
            r#"["/old/node_modules/deepseek-acp/index.js","/old/node_modules/@deepseek-ai/dsh/main.js"]"#,
            None,
        ),
        ("custom", "pi", "/custom/my-pi-wrapper", "[]", None),
        ("different-host", "custom", "deepseek-acp", "[]", None),
    ];
    for (id, host, command, args, _) in cases {
        sqlx::query("INSERT INTO agent_configs(id,name,host_id,protocol,enabled,command,args_json,env_json,created_at,updated_at) VALUES(?1,?1,?2,'acp',0,?3,?4,?5,?6,?6)")
            .bind(id).bind(host).bind(command).bind(args).bind(r#"{"TOKEN":"keep-secret","CUSTOM":"keep"}"#).bind(CREATED)
            .execute(&mut *connection).await.unwrap();
    }
    drop(connection);
    pool.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    for (id, host, command, args, catalog) in cases {
        let config = store.get_agent_config(id).await.unwrap();
        assert_eq!(config.catalog_id.as_deref(), catalog, "{id}");
        assert_eq!(
            (
                config.id.as_str(),
                config.name.as_str(),
                config.host_id.as_str()
            ),
            (id, id, host)
        );
        assert_eq!(config.command, command);
        assert_eq!(
            config.args,
            serde_json::from_str::<Vec<String>>(args).unwrap()
        );
        assert_eq!(config.env["TOKEN"], "keep-secret");
        assert!(!config.enabled);
        assert_eq!(config.updated_at, CREATED);
    }
}

fn config() -> AgentConfig {
    AgentConfig {
        catalog_id: None,
        id: "agent-config".into(),
        name: "Local dsh".into(),
        host_id: "deepseek-harness".into(),
        protocol: SessionProtocol::Acp,
        enabled: true,
        command: "dsh".into(),
        args: vec![
            "--profile".into(),
            "acp".into(),
            "argument with spaces".into(),
        ],
        env: BTreeMap::from([("TOKEN".into(), "sensitive-test-value".into())]),
        created_at: CREATED.into(),
        updated_at: CREATED.into(),
    }
}

fn session(workspace: &TestWorkspace, session_id: &str) -> NewManagedSession {
    NewManagedSession {
        session_id: session_id.into(),
        agent_config_id: config().id,
        cwd: workspace._temp.path().to_string_lossy().into_owned(),
        title: "Independent session".into(),
        created_at: CREATED.into(),
    }
}

#[tokio::test]
async fn version_ten_migration_preserves_existing_session_identity() {
    let workspace = TestWorkspace::new().await;
    tokio::fs::create_dir_all(workspace.database.parent().unwrap())
        .await
        .unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&workspace.database)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    let mut connection = pool.acquire().await.unwrap();
    connection.ensure_migrations_table().await.unwrap();
    for migration in MIGRATOR.iter().filter(|migration| migration.version <= 10) {
        connection.apply(migration).await.unwrap();
    }
    sqlx::query("INSERT INTO host_sessions (id, host_id, host_session_id, display_title, created_at, updated_at) VALUES ('old-local', 'pi', 'old-agent', 'Existing title', ?1, ?1)")
        .bind(CREATED).execute(&mut *connection).await.unwrap();
    drop(connection);
    pool.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    let record = store.get_session("old-local").await.unwrap();
    assert_eq!(record.host_session_id, "old-agent");
    assert_eq!(record.title, "Existing title");
    assert_eq!(record.created_at, CREATED);
    assert_eq!(record.management, SessionManagement::External);
    assert!(store.list_agent_configs().await.unwrap().is_empty());
}

#[tokio::test]
async fn managed_records_survive_reopen_without_a_feedback_request() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    assert_eq!(store.save_agent_config(config()).await.unwrap(), config());
    let record = store
        .create_managed_session(session(&workspace, "local-one"))
        .await
        .unwrap();
    assert_eq!(record.session_id, "local-one");
    assert_eq!(record.host_session_id, record.session_id);
    assert_eq!(record.host_id, config().host_id);
    assert!(matches!(
        record.management,
        SessionManagement::Managed {
            remote_session_id: None,
            ..
        }
    ));
    store
        .bind_remote_session("local-one", "remote-one", UPDATED)
        .await
        .unwrap();
    store.close().await;

    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    assert_eq!(
        store.get_agent_config("agent-config").await.unwrap(),
        config()
    );
    let records = store.list_managed_sessions().await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].updated_at, UPDATED);
    assert_eq!(records[0].created_at, CREATED);
    assert!(
        matches!(&records[0].management, SessionManagement::Managed {
        remote_session_id: Some(id), ..
    } if id == "remote-one")
    );
}

#[tokio::test]
async fn existing_host_session_retains_external_identity() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO host_sessions (id, host_id, host_session_id, created_at, updated_at) \
         VALUES ('legacy-local', 'pi', 'legacy-correlation', ?1, ?1)",
    )
    .bind(CREATED)
    .execute(&store.pool)
    .await
    .unwrap();
    let record = store.get_session("legacy-local").await.unwrap();
    assert_eq!(record.management, SessionManagement::External);
    assert_eq!(record.host_session_id, "legacy-correlation");
    assert!(store.list_managed_sessions().await.unwrap().is_empty());
    assert_eq!(
        store
            .bind_remote_session("legacy-local", "remote", UPDATED)
            .await,
        Err(SessionRepositoryError::SessionNotFound)
    );
}

#[tokio::test]
async fn remote_binding_cannot_replace_a_previous_agent_context() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    store.save_agent_config(config()).await.unwrap();
    store
        .create_managed_session(session(&workspace, "local-one"))
        .await
        .unwrap();
    store
        .create_managed_session(session(&workspace, "local-two"))
        .await
        .unwrap();
    store
        .bind_remote_session("local-one", "remote-one", UPDATED)
        .await
        .unwrap();
    store
        .bind_remote_session("local-one", "remote-one", UPDATED)
        .await
        .unwrap();
    assert_eq!(
        store
            .bind_remote_session("local-one", "replacement", UPDATED)
            .await,
        Err(SessionRepositoryError::Conflict)
    );
    // Remote ids are scoped to ACP instances, not globally unique.
    store
        .bind_remote_session("local-two", "remote-one", UPDATED)
        .await
        .unwrap();
    assert!(
        matches!(store.get_session("local-one").await.unwrap().management,
        SessionManagement::Managed { remote_session_id: Some(id), .. } if id == "remote-one")
    );
}

#[tokio::test]
async fn configuration_edits_preserve_creation_and_protect_bound_host_identity() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    store.save_agent_config(config()).await.unwrap();
    store
        .create_managed_session(session(&workspace, "local-one"))
        .await
        .unwrap();
    assert_eq!(
        store.delete_agent_config("agent-config").await,
        Err(SessionRepositoryError::AgentConfigInUse)
    );
    let mut edited = config();
    edited.host_id = "pi".into();
    assert_eq!(
        store.save_agent_config(edited).await,
        Err(SessionRepositoryError::AgentConfigInUse)
    );
    let mut edited = config();
    edited.enabled = false;
    edited.created_at = UPDATED.into();
    edited.updated_at = UPDATED.into();
    edited.args.push("--new-option".into());
    let saved = store.save_agent_config(edited).await.unwrap();
    assert_eq!(saved.created_at, CREATED);
    assert_eq!(saved.updated_at, UPDATED);
    assert_eq!(
        store
            .create_managed_session(session(&workspace, "disabled"))
            .await,
        Err(SessionRepositoryError::AgentConfigDisabled)
    );
    assert_eq!(
        store.get_session("disabled").await,
        Err(SessionRepositoryError::SessionNotFound)
    );
}

#[tokio::test]
async fn invalid_creation_and_identity_conflicts_leave_no_partial_records() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    assert_eq!(
        store
            .create_managed_session(session(&workspace, "missing-config"))
            .await,
        Err(SessionRepositoryError::AgentConfigNotFound)
    );
    store.save_agent_config(config()).await.unwrap();
    let mut relative = session(&workspace, "relative");
    relative.cwd = "relative/project".into();
    assert_eq!(
        store.create_managed_session(relative).await,
        Err(SessionRepositoryError::InvalidInput)
    );
    store
        .create_managed_session(session(&workspace, "one"))
        .await
        .unwrap();
    assert_eq!(
        store
            .create_managed_session(session(&workspace, "one"))
            .await,
        Err(SessionRepositoryError::Conflict)
    );
    assert_eq!(store.list_managed_sessions().await.unwrap().len(), 1);
    let mut invalid = config();
    invalid.env.insert("INVALID=KEY".into(), "value".into());
    assert_eq!(
        store.save_agent_config(invalid).await,
        Err(SessionRepositoryError::InvalidInput)
    );
    assert_eq!(
        store.get_agent_config("agent-config").await.unwrap(),
        config()
    );
    let mut unused = config();
    unused.id = "unused".into();
    store.save_agent_config(unused).await.unwrap();
    assert_eq!(store.list_agent_configs().await.unwrap().len(), 2);
    store.delete_agent_config("unused").await.unwrap();
    assert_eq!(
        store.get_agent_config("unused").await,
        Err(SessionRepositoryError::AgentConfigNotFound)
    );
}

#[tokio::test]
async fn zero_feedback_session_can_be_listed_renamed_pinned_and_archived() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    store.save_agent_config(config()).await.unwrap();
    let record = store
        .create_managed_session(session(&workspace, "zero-feedback"))
        .await
        .unwrap();
    let summaries = store
        .list_host_sessions(HostSessionQuery {
            archived: false,
            search: None,
        })
        .await
        .unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].session_id, record.session_id);
    assert_eq!(summaries[0].management, record.management);
    assert_eq!(summaries[0].request_count, 0);
    assert_eq!(summaries[0].pending_count, 0);
    assert_eq!(summaries[0].title, "Independent session");
    assert_eq!(summaries[0].updated_at, CREATED);
    assert_eq!(
        summaries[0].source_hint,
        Some(workspace._temp.path().to_string_lossy().into_owned())
    );

    let renamed = store
        .rename_host_session(
            &record.host_id,
            &record.host_session_id,
            "Searchable project",
            UPDATED,
        )
        .await
        .unwrap();
    assert_eq!(renamed.updated_at, UPDATED);
    assert_eq!(renamed.title, "Searchable project");
    let pinned = store
        .set_host_session_pinned(&record.host_id, &record.host_session_id, Some(UPDATED))
        .await
        .unwrap();
    assert_eq!(pinned.pinned_at.as_deref(), Some(UPDATED));
    store
        .archive_host_session(&record.host_id, &record.host_session_id, UPDATED)
        .await
        .unwrap();
    assert!(
        store
            .list_host_sessions(HostSessionQuery {
                archived: false,
                search: None
            })
            .await
            .unwrap()
            .is_empty()
    );
    let archived = store
        .list_host_sessions(HostSessionQuery {
            archived: true,
            search: Some("Searchable".into()),
        })
        .await
        .unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].request_count, 0);
    assert_eq!(archived[0].pinned_at, None);
    store
        .unarchive_host_session(&record.host_id, &record.host_session_id, UPDATED)
        .await
        .unwrap();
}

#[tokio::test]
async fn deleting_last_feedback_keeps_managed_session_and_remote_binding() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    store.save_agent_config(config()).await.unwrap();
    let record = store
        .create_managed_session(session(&workspace, "keep-session"))
        .await
        .unwrap();
    store
        .bind_remote_session(&record.session_id, "keep-agent-context", UPDATED)
        .await
        .unwrap();
    let application = store.clone().into_application();
    let request_id = Uuid::now_v7().to_string();
    let mut input = workspace.request(request_id.clone());
    input.host_id = Some(record.host_id.clone());
    input.host_session_id = record.host_session_id.clone();
    application
        .request_managed_feedback(
            &rambledesk_core::ManagedFeedbackScope::from_session(&record).unwrap(),
            input,
        )
        .await
        .unwrap();
    let summaries = application.list_host_sessions().await.unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].session_id, record.session_id);
    assert_eq!(summaries[0].request_count, 1);
    application
        .cancel_feedback(CancelFeedbackInput {
            request_id: request_id.clone(),
            reason: "Fixture cleanup".into(),
        })
        .await
        .unwrap();
    application
        .archive_host_session(HostSessionInput {
            host_id: record.host_id,
            host_session_id: record.host_session_id,
        })
        .await
        .unwrap();
    application
        .delete_feedback_request(DeleteFeedbackRequestInput { request_id })
        .await
        .unwrap();
    let summaries = application
        .list_archived_host_sessions(ListHostSessionsInput::default())
        .await
        .unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].request_count, 0);
    assert_eq!(summaries[0].pending_count, 0);
    assert_eq!(summaries[0].session_id, record.session_id);
    assert!(
        matches!(&summaries[0].management, SessionManagement::Managed { remote_session_id: Some(id), .. } if id == "keep-agent-context")
    );
    assert!(store.get_session(&record.session_id).await.is_ok());
}
