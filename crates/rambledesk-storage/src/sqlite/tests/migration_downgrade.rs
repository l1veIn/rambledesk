use std::{borrow::Cow, collections::BTreeMap};

use rambledesk_core::{
    AgentConfig, NewManagedSession, NewSessionActivity, SessionActivityKind,
    SessionActivityRepository, SessionManagement, SessionProtocol, SessionRepository,
};

use super::*;

// LF checksums from tag v0.3.3 (e88217283f92a58ba1390cb2c5d4cd3534a7e338).
// Pin the released contract so editing a historical SQL file cannot make the
// compatibility test silently use a different "old version".
const V033_CHECKSUMS: [&str; 10] = [
    "4f9fffaa32b9f1b727ab4dda52a37f4fffcf5a968971c5db457711328e3275f1c14aba4ff5760321e39b91e3667f7f4d",
    "9924296b51f83c3a8679986840b23c8ab6c2c8998dab109c3218a3c33c0affe09356f1c7cc470954d84fc6728ab38348",
    "a2fbc77ceb9a0d52866549f3faae81ff817d59c71c56b0523ff754206645956627997c94e12a85abe3c8ed12ea0ac5cc",
    "26720db49563098ed9d3ead808ebf249fe75a9140d28b49e60155e0163dcb7230fd89d6574c1630286502bfe362bdc6b",
    "e33a7db36d13e1f47c600997e45f410405325acf9b57bab339a8de2fb9d75dcd748c21461382a14e93fd0cbeb91d2239",
    "03b920d90cc21b197174ec92eae5b16cc93039f33a576afe255e3d3f5bee6a6b74e12becdbd783d22806aa1655c7cffd",
    "01925032488aa158927f42c3cf0c590747d62a4d53af2cb919dfb165a2f9e78bb514984fcf899bdf3399b581ffe1547e",
    "d79063896651ed119a3eb124ef0cb4f655f16f209f1db325e89e3760f6ad6a393465c2c764c71aa7afd597d3d2df6177",
    "f5bf20625fa3ddd08613c32f2980cb7f0d8d5b704560252e9268088665e5116ef0a5955d1201224d2cf5d0045b4a6716",
    "9bc7e3369c5c95abdcc595899699f4e6d85a300d5f15f710117776b78e16e753bf96ea2674c11d3a2b3fdc9af49355c4",
];
// LF checksums of extensions 11..=20 from the released v0.4.0 tag.
const V040_EXTENSION_CHECKSUMS: [&str; 10] = [
    "0f38c4cece52d0f29eb672cad895e3d2a57bcd2019ae5c6342e98f04baf459aeed5b2ce5bdcb611a9e1ebcef64ed4be3",
    "6e664219505ecadb09ca645840fda40f485ac79483788e21dab33748cd288d159a8f654fe7a7c78e0c4fd0f4a7b4569f",
    "72c58f0c21962a1440cb9ae5b1739f415e7aea2b9c9200c7ca9512d9d5bb2895ac4b06f4a89e4eb2a7a39a0bf6dfe26f",
    "f87e73c51f8713a6260c4e28318ecbb2d7855dd40473f43a979f1f9aaddcad2ab4fdc6510bdf135a1646fcf9d6565fa9",
    "f65d774c15720ef3cb6ee2bf4b60386017f6dd56777d4351062ef4cf35e57bc06b3045b7aa26566b5069e6d0ae1170b8",
    "01e4cfdde1650825be2679f03c8e0d1b3df8ea3f91f67e54c3f876f38ce9630ac17fce260d1b2c5f55d9aa5362f49853",
    "0f574009018230ff6900b011d36722822ea198edfc4332026d869349e5610168a4bb9cbc7636488f818114a048d041e8",
    "0ef6f2186a766cde16ee83b725832076162bb2eacd0646daeb4c658c41907fd6a1661a78da1fcd2d8346b039fe09eabc",
    "a5ca635709567e7ee8c631edb60543d02139005337207eee670f1210a21a608c4d5e3e6c98ef2774353b83c3918b0f78",
    "5b4a4e42a85f485e58b6b17e7775a3b37d005ffb7b32898eb669ef3351a653d04aa7d0ceb7eb4cbabacbf2e0f6483078",
];
const NOW: &str = "2026-09-06T01:00:00Z";

fn v033_migrator() -> Migrator {
    let migrations = MIGRATOR
        .iter()
        .filter(|migration| migration.version <= 10)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(migrations.len(), V033_CHECKSUMS.len());
    for (migration, checksum) in migrations.iter().zip(V033_CHECKSUMS) {
        let normalized = migration.sql.replace("\r\n", "\n");
        assert_eq!(hex::encode(Sha384::digest(normalized.as_bytes())), checksum);
    }
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

async fn pool(workspace: &TestWorkspace) -> SqlitePool {
    tokio::fs::create_dir_all(workspace.database.parent().unwrap())
        .await
        .unwrap();
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&workspace.database)
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await
        .unwrap()
}

async fn open_as_v033(workspace: &TestWorkspace) -> SqlitePool {
    let pool = pool(workspace).await;
    let migrator = v033_migrator();
    // The released startup's explicit newer-schema gate followed by SQLx's
    // complete checksum/unknown-version verification, not ignore_missing.
    migration_compat::repair_line_ending_checksums(&pool, &migrator)
        .await
        .unwrap();
    assert!(applied_migration_version(&pool).await.unwrap() <= 10);
    migrator.run(&pool).await.unwrap();
    pool
}

async fn legacy_insert(pool: &SqlitePool, id: &str) {
    // Original v0.3.3 column lists from request_ops.rs; new optional columns
    // must remain omitted exactly as they are in the shipped binary.
    sqlx::query(
        "INSERT INTO host_sessions (id, host_id, host_session_id, created_at, updated_at)
         VALUES (?1, 'legacy', ?1, ?2, ?2)
         ON CONFLICT(host_id, host_session_id) DO UPDATE SET
             updated_at = excluded.updated_at, archived_at = NULL",
    )
    .bind(id)
    .bind(NOW)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO feedback_requests
         (id, host_session_record_id, title, what_happened, source_hint, status,
          input_hash, allow_finish, final_summary, created_at, updated_at)
         VALUES (?1, ?1, 'Legacy request', 'Review', NULL, 'waiting', 'hash', 0, NULL, ?2, ?2)
         ON CONFLICT(id) DO NOTHING",
    )
    .bind(id)
    .bind(NOW)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_acp(workspace: &TestWorkspace, store: &SqliteFeedbackStore) {
    store
        .save_agent_config(AgentConfig {
            catalog_id: Some("dsh".into()),
            id: "config".into(),
            name: "Test".into(),
            host_id: "dsh".into(),
            protocol: SessionProtocol::Acp,
            enabled: true,
            command: "agent".into(),
            args: vec!["--acp".into()],
            env: BTreeMap::from([("KEY".into(), "fixture-value".into())]),
            created_at: NOW.into(),
            updated_at: NOW.into(),
        })
        .await
        .unwrap();
    store
        .create_managed_session(NewManagedSession {
            session_id: "managed".into(),
            agent_config_id: "config".into(),
            cwd: workspace._temp.path().to_string_lossy().into_owned(),
            title: "Managed session".into(),
            created_at: NOW.into(),
        })
        .await
        .unwrap();
    store
        .bind_remote_session("managed", "remote", NOW)
        .await
        .unwrap();
    store
        .append_activity(NewSessionActivity {
            id: "activity".into(),
            session_id: "managed".into(),
            turn_id: Some("turn".into()),
            kind: SessionActivityKind::AgentMessage,
            text: "Retained response".into(),
            content: None,
            tool_call_id: None,
            created_at: NOW.into(),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn v040_extension_20_upgrades_to_22_preserving_existing_data_and_history() {
    let workspace = TestWorkspace::new().await;
    let released = pool(&workspace).await;
    let migrations = MIGRATOR
        .iter()
        .filter(|migration| migration.version <= 20)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(migrations.len(), 20);
    for (migration, checksum) in migrations
        .iter()
        .zip(V033_CHECKSUMS.into_iter().chain(V040_EXTENSION_CHECKSUMS))
    {
        assert_eq!(
            hex::encode(Sha384::digest(
                migration.sql.replace("\r\n", "\n").as_bytes()
            )),
            checksum
        );
    }
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
    .run(&released)
    .await
    .unwrap();
    // Reproduce v0.4.0's separate extension ledger; this is an upgrade fixture,
    // not a claim that the v0.4.0 binary accepts extensions newer than 20.
    sqlx::raw_sql(
        "CREATE TABLE _rambledesk_extensions (
        version BIGINT PRIMARY KEY, description TEXT NOT NULL,
        installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
        success BOOLEAN NOT NULL, checksum BLOB NOT NULL, execution_time BIGINT NOT NULL
    );
    INSERT INTO _rambledesk_extensions SELECT * FROM _sqlx_migrations WHERE version > 10;
    DELETE FROM _sqlx_migrations WHERE version > 10;",
    )
    .execute(&released)
    .await
    .unwrap();
    let extension_version: i64 =
        sqlx::query_scalar("SELECT max(version) FROM _rambledesk_extensions")
            .fetch_one(&released)
            .await
            .unwrap();
    assert_eq!(extension_version, 20);
    assert_eq!(applied_migration_version(&released).await.unwrap(), 10);
    type History = (i64, String, String, bool, Vec<u8>, i64);
    const HISTORY: &str = "SELECT version,description,installed_on,success,checksum,execution_time FROM _rambledesk_extensions WHERE version <= 20 ORDER BY version";
    let history: Vec<History> = sqlx::query_as(HISTORY).fetch_all(&released).await.unwrap();
    legacy_insert(&released, "v040-request").await;
    sqlx::query(
        "INSERT INTO drafts(request_id,document_json,body_markdown,revision,updated_at)
        VALUES('v040-request',NULL,'Unsubmitted v0.4.0 draft',3,?1)",
    )
    .bind(NOW)
    .execute(&released)
    .await
    .unwrap();
    sqlx::query("UPDATE feedback_requests SET revision=3 WHERE id='v040-request'")
        .execute(&released)
        .await
        .unwrap();
    let material = b"# Original v0.4.0 material";
    sqlx::query("INSERT INTO request_attachments(id,request_id,file_name,byte_size,media_type,sha256,position,contents,created_at)
        VALUES('v040-material','v040-request','original.md',?1,'text/markdown',?2,0,?3,?4)")
        .bind(material.len() as i64).bind(hex::encode(Sha256::digest(material)))
        .bind(material.as_slice()).bind(NOW).execute(&released).await.unwrap();
    type Draft = (Option<String>, String, i64, String);
    const DRAFT: &str = "SELECT document_json,body_markdown,revision,updated_at FROM drafts WHERE request_id='v040-request'";
    let draft: Draft = sqlx::query_as(DRAFT).fetch_one(&released).await.unwrap();
    released.close().await;

    for _ in 0..2 {
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .unwrap();
        let versions: Vec<i64> =
            sqlx::query_scalar("SELECT version FROM _rambledesk_extensions ORDER BY version")
                .fetch_all(&store.pool)
                .await
                .unwrap();
        assert_eq!(versions, (11..=22).collect::<Vec<_>>());
        assert_eq!(
            sqlx::query_as::<_, History>(HISTORY)
                .fetch_all(&store.pool)
                .await
                .unwrap(),
            history
        );
        assert_eq!(
            sqlx::query_as::<_, Draft>(DRAFT)
                .fetch_one(&store.pool)
                .await
                .unwrap(),
            draft
        );
        let existing = store.get_workspace("v040-request").await.unwrap();
        assert_eq!(existing.request.title, "Legacy request");
        assert!(existing.workbench.is_none());
        assert_eq!(
            store
                .read_request_attachment("v040-request", "v040-material")
                .await
                .unwrap(),
            material
        );
        assert_eq!(applied_migration_version(&store.pool).await.unwrap(), 10);
        assert!(
            sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(&store.pool)
                .await
                .unwrap()
                .is_empty()
        );
        store.close().await;
    }
}

#[tokio::test]
async fn upgrade_downgrade_upgrade_preserves_legacy_writes_and_acp_data() {
    let workspace = TestWorkspace::new().await;
    let old = open_as_v033(&workspace).await;
    legacy_insert(&old, "before-upgrade").await;
    old.close().await;

    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    seed_acp(&workspace, &store).await;
    let config = store.get_agent_config("config").await.unwrap();
    let session = store.get_session("managed").await.unwrap();
    store.close().await;

    let old = open_as_v033(&workspace).await;
    legacy_insert(&old, "during-downgrade").await;
    sqlx::query("UPDATE host_sessions SET display_title='Renamed in 0.3.3',archived_at=?1,pinned_at=NULL,updated_at=?1 WHERE id='before-upgrade'")
        .bind(NOW).execute(&old).await.unwrap();
    // The old navigation uses this inner join; ACP-only sessions remain hidden.
    let visible: Vec<String> = sqlx::query_scalar(
        "SELECT hs.host_session_id FROM host_sessions hs
         JOIN feedback_requests r ON r.host_session_record_id = hs.id
         WHERE hs.archived_at IS NULL GROUP BY hs.id ORDER BY hs.host_session_id",
    )
    .fetch_all(&old)
    .await
    .unwrap();
    assert_eq!(visible, ["during-downgrade"]);
    sqlx::query("INSERT INTO drafts (request_id, document_json, body_markdown, revision, updated_at)
        VALUES ('before-upgrade', NULL, 'Saved in 0.3.3', 1, ?1)
        ON CONFLICT(request_id) DO UPDATE SET document_json=excluded.document_json,
            body_markdown=excluded.body_markdown,revision=excluded.revision,updated_at=excluded.updated_at")
        .bind(NOW).execute(&old).await.unwrap();
    old.close().await;

    for _ in 0..2 {
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .unwrap();
        assert_eq!(store.get_agent_config("config").await.unwrap(), config);
        assert_eq!(store.get_session("managed").await.unwrap(), session);
        let renamed = store.get_session("before-upgrade").await.unwrap();
        assert_eq!(renamed.title, "Renamed in 0.3.3");
        let archived: Option<String> =
            sqlx::query_scalar("SELECT archived_at FROM host_sessions WHERE id='before-upgrade'")
                .fetch_one(&store.pool)
                .await
                .unwrap();
        assert_eq!(archived.as_deref(), Some(NOW));
        assert_eq!(
            store
                .get_session("during-downgrade")
                .await
                .unwrap()
                .management,
            SessionManagement::External
        );
        assert_eq!(
            store
                .get_request("during-downgrade")
                .await
                .unwrap()
                .managed_session_id,
            None
        );
        let text: String =
            sqlx::query_scalar("SELECT text FROM session_activity WHERE id='activity'")
                .fetch_one(&store.pool)
                .await
                .unwrap();
        assert_eq!(text, "Retained response");
        let draft: String = sqlx::query_scalar(
            "SELECT body_markdown FROM drafts WHERE request_id='before-upgrade'",
        )
        .fetch_one(&store.pool)
        .await
        .unwrap();
        assert_eq!(draft, "Saved in 0.3.3");
        assert_eq!(applied_migration_version(&store.pool).await.unwrap(), 10);
        store.close().await;
    }
}

#[tokio::test]
async fn existing_development_ledger_is_relocated_without_replaying_migrations() {
    let workspace = TestWorkspace::new().await;
    let development = pool(&workspace).await;
    MIGRATOR.run(&development).await.unwrap();
    assert_eq!(applied_migration_version(&development).await.unwrap(), 22);
    legacy_insert(&development, "retained").await;
    development.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    assert_eq!(applied_migration_version(&store.pool).await.unwrap(), 10);
    let extensions: i64 = sqlx::query_scalar("SELECT count(*) FROM _rambledesk_extensions")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(extensions, 12);
    assert!(store.get_request("retained").await.is_ok());
    store.close().await;
    open_as_v033(&workspace).await.close().await;
}

#[tokio::test]
async fn media_extension_preserves_material_rows_and_legacy_writes_across_downgrade() {
    use base64::Engine;
    type MaterialRow = (
        String,
        i64,
        String,
        String,
        i64,
        Vec<u8>,
        String,
        Option<String>,
        Option<String>,
    );
    const MATERIAL: &str = "SELECT file_name,byte_size,media_type,sha256,position,contents,created_at,draft_path,published_path FROM request_attachments WHERE id='legacy-material'";
    let workspace = TestWorkspace::new().await;
    let prior = pool(&workspace).await;
    let migrator = Migrator {
        migrations: Cow::Owned(
            MIGRATOR
                .iter()
                .filter(|migration| migration.version <= 21)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    migrator.run(&prior).await.unwrap();
    legacy_insert(&prior, "before-media").await;
    let path = workspace._temp.path().join("old-material.md");
    let original = b"# Preserved material";
    tokio::fs::write(&path, original).await.unwrap();
    sqlx::query("INSERT INTO request_attachments(id,request_id,file_name,byte_size,media_type,sha256,position,contents,created_at,draft_path,published_path) VALUES('legacy-material','before-media','old-material.md',?1,'text/markdown',?2,0,x'',?3,?4,?4)")
        .bind(original.len() as i64).bind(hex::encode(Sha256::digest(original))).bind(NOW)
        .bind(path.to_string_lossy().as_ref()).execute(&prior).await.unwrap();
    let before: MaterialRow = sqlx::query_as(MATERIAL).fetch_one(&prior).await.unwrap();
    prior.close().await;

    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_as::<_, MaterialRow>(MATERIAL)
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        before
    );
    // A real PCM16 WAV exercises the newly permitted MIME through production creation.
    let mut wav = b"RIFF".to_vec();
    wav.extend(16036u32.to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(8000u32.to_le_bytes());
    wav.extend(16000u32.to_le_bytes());
    wav.extend(2u16.to_le_bytes());
    wav.extend(16u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(16000u32.to_le_bytes());
    wav.resize(16044, 0);
    let app = store.clone().into_application();
    let mut input = workspace.request(Uuid::now_v7().to_string());
    input.attachments = vec![RequestAttachmentInput {
        file_name: "clip.wav".into(),
        markdown: None,
        contents_base64: Some(base64::engine::general_purpose::STANDARD.encode(&wav)),
        path: None,
    }];
    let created = app.request_feedback(input).await.unwrap();
    let material = app
        .get_feedback_workspace(created.request_id.clone())
        .await
        .unwrap()
        .request_attachments
        .remove(0);
    assert_eq!(material.media_type, "audio/wav");
    store.close().await;

    let old = open_as_v033(&workspace).await;
    assert_eq!(
        sqlx::query_as::<_, MaterialRow>(MATERIAL)
            .fetch_one(&old)
            .await
            .unwrap(),
        before
    );
    let media_type: String =
        sqlx::query_scalar("SELECT media_type FROM request_attachments WHERE id=?1")
            .bind(&material.attachment_id)
            .fetch_one(&old)
            .await
            .unwrap();
    assert_eq!(media_type, "audio/wav");
    legacy_insert(&old, "during-media-downgrade").await;
    sqlx::query("INSERT INTO request_attachments(id,request_id,file_name,byte_size,media_type,sha256,position,contents,created_at) VALUES('during-material','during-media-downgrade','old-material.md',?1,'text/markdown',?2,0,?3,?4)")
        .bind(original.len() as i64).bind(hex::encode(Sha256::digest(original))).bind(original.as_slice())
        .bind(NOW).execute(&old).await.unwrap();
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&old)
            .await
            .unwrap()
            .is_empty()
    );
    old.close().await;

    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_as::<_, MaterialRow>(MATERIAL)
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        before
    );
    assert_eq!(
        store
            .read_request_attachment(&created.request_id, &material.attachment_id)
            .await
            .unwrap(),
        wav
    );
    assert_eq!(
        store
            .read_request_attachment("during-media-downgrade", "during-material")
            .await
            .unwrap(),
        original
    );
    assert_eq!(applied_migration_version(&store.pool).await.unwrap(), 10);
    store.close().await;
}

#[tokio::test]
async fn corrupt_or_future_extension_history_is_rejected_without_relocation() {
    for future in [false, true] {
        let workspace = TestWorkspace::new().await;
        let development = pool(&workspace).await;
        MIGRATOR.run(&development).await.unwrap();
        if future {
            sqlx::query(
                "INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time)
                VALUES (23,'future',TRUE,X'00',0)",
            )
            .execute(&development)
            .await
            .unwrap();
        } else {
            sqlx::query("UPDATE _sqlx_migrations SET checksum=X'00' WHERE version=18")
                .execute(&development)
                .await
                .unwrap();
        }
        development.close().await;
        assert!(
            SqliteFeedbackStore::connect(&workspace.database)
                .await
                .is_err()
        );
        let check = pool(&workspace).await;
        assert_eq!(
            applied_migration_version(&check).await.unwrap(),
            if future { 23 } else { 22 }
        );
        let extensions: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='_rambledesk_extensions')",
        )
        .fetch_one(&check)
        .await
        .unwrap();
        assert!(!extensions);
        check.close().await;
    }
}

#[tokio::test]
async fn future_separate_extension_and_dirty_history_still_block_startup() {
    for future in [false, true] {
        let workspace = TestWorkspace::new().await;
        let store = SqliteFeedbackStore::connect(&workspace.database)
            .await
            .unwrap();
        if future {
            sqlx::query("INSERT INTO _rambledesk_extensions(version,description,success,checksum,execution_time)
                VALUES(23,'future',TRUE,X'00',0)").execute(&store.pool).await.unwrap();
        } else {
            sqlx::query("UPDATE _rambledesk_extensions SET success=FALSE WHERE version=18")
                .execute(&store.pool)
                .await
                .unwrap();
        }
        store.close().await;
        let error = match SqliteFeedbackStore::connect(&workspace.database).await {
            Ok(_) => panic!("invalid extension ledger accepted"),
            Err(error) => error,
        };
        assert!(if future {
            matches!(error, StorageOpenError::NewerDatabase { applied: 23, .. })
        } else {
            matches!(
                error,
                StorageOpenError::Migrate(sqlx::migrate::MigrateError::Dirty(18))
            )
        });
    }
}

#[tokio::test]
async fn failed_extension_sql_rolls_back_development_ledger_relocation() {
    use sqlx::migrate::Migrate;
    let workspace = TestWorkspace::new().await;
    let development = pool(&workspace).await;
    let mut connection = development.acquire().await.unwrap();
    connection.ensure_migrations_table().await.unwrap();
    for migration in MIGRATOR.iter().filter(|migration| migration.version <= 17) {
        connection.apply(migration).await.unwrap();
    }
    // Simulate schema damage that makes extension18 fail after bookkeeping
    // relocation has begun. Its partial ALTERs and the ledger move must roll back.
    sqlx::query("CREATE INDEX agent_configs_catalog ON agent_configs(name)")
        .execute(&mut *connection)
        .await
        .unwrap();
    drop(connection);
    development.close().await;
    assert!(
        SqliteFeedbackStore::connect(&workspace.database)
            .await
            .is_err()
    );
    let check = pool(&workspace).await;
    assert_eq!(applied_migration_version(&check).await.unwrap(), 17);
    let added_column: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('agent_configs') WHERE name='catalog_id')",
    )
    .fetch_one(&check)
    .await
    .unwrap();
    assert!(!added_column);
    let extensions: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='_rambledesk_extensions')",
    )
    .fetch_one(&check)
    .await
    .unwrap();
    assert!(!extensions);
}

#[tokio::test]
async fn explicit_legacy_session_deletion_cascades_extensions_without_foreign_key_damage() {
    let workspace = TestWorkspace::new().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    seed_acp(&workspace, &store).await;
    sqlx::query("INSERT INTO feedback_requests(id,host_session_record_id,managed_session_id,title,what_happened,status,resolution,input_hash,created_at,updated_at,completed_at)
        VALUES('managed-feedback','managed','managed','Review','Fixture','completed','approved','hash',?1,?1,?1)")
        .bind(NOW).execute(&store.pool).await.unwrap();
    sqlx::query("UPDATE host_sessions SET archived_at=?1 WHERE id='managed'")
        .bind(NOW)
        .execute(&store.pool)
        .await
        .unwrap();
    store.close().await;
    let old = open_as_v033(&workspace).await;
    let mut transaction = old.begin_with("BEGIN IMMEDIATE").await.unwrap();
    // The deletion order from v0.3.3 session_ops.rs, after its archived/terminal
    // checks. This is an explicit user deletion, never performed on startup.
    for query in [
        "DELETE FROM feedback_results WHERE request_id='managed-feedback'",
        "DELETE FROM submission_plans WHERE request_id='managed-feedback'",
        "DELETE FROM feedback_requests WHERE id='managed-feedback'",
        "DELETE FROM host_sessions WHERE id='managed'",
    ] {
        sqlx::query(query).execute(&mut *transaction).await.unwrap();
    }
    transaction.commit().await.unwrap();
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&old)
            .await
            .unwrap()
            .is_empty()
    );
    old.close().await;
    let store = SqliteFeedbackStore::connect(&workspace.database)
        .await
        .unwrap();
    assert!(store.get_session("managed").await.is_err());
    assert!(store.get_agent_config("config").await.is_ok());
    let activity_count: i64 = sqlx::query_scalar("SELECT count(*) FROM session_activity")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(activity_count, 0);
}
