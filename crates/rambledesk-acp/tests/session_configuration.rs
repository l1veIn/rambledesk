mod support;

use rambledesk_core::*;
use rambledesk_storage::SqliteFeedbackStore;
use std::{sync::Arc, time::Duration};
use support::{create, id, setup as setup_fixture, wait_for};

async fn setup(
    mode: &str,
) -> (
    tempfile::TempDir,
    Arc<SqliteFeedbackStore>,
    SessionApplication,
    ManagedSessionInput,
) {
    let (dir, store, app, config) = setup_fixture("configuration", mode).await;
    let session = create(&app, &dir, &config, "Session").await;
    assert_eq!(
        session.runtime.connection,
        SessionConnectionState::Connected,
        "{:?}",
        session.runtime.last_error
    );
    (dir, store, app, id(&session))
}
fn change(id: &ManagedSessionInput, change: SessionConfigChange) -> SetManagedSessionConfigInput {
    SetManagedSessionConfigInput {
        session_id: id.session_id.clone(),
        change,
    }
}
fn select(
    configuration: &SessionConfiguration,
    category: &str,
    value: &str,
) -> SessionConfigChange {
    let option = configuration
        .options
        .iter()
        .find(|option| option.category.as_deref() == Some(category))
        .unwrap_or_else(|| panic!("Missing {category} option: {configuration:?}"));
    SessionConfigChange {
        config_id: option.id.clone(),
        value: SessionConfigValue::Select {
            value: value.into(),
        },
    }
}

async fn configuration(app: &SessionApplication, id: &ManagedSessionInput) -> SessionConfiguration {
    app.get_session(id.clone())
        .await
        .unwrap()
        .runtime
        .configuration
}

#[tokio::test]
async fn standard_and_legacy_catalogs_expose_the_same_application_contract_and_setter() {
    let mut configurations = vec![];
    for fixture in ["legacy", "native-catalog"] {
        let (_dir, store, app, id) = setup(fixture).await;
        let initial = configuration(&app, &id).await;
        assert_eq!(
            serde_json::to_value(&initial)
                .unwrap()
                .as_object()
                .unwrap()
                .len(),
            1
        );
        for (category, value) in [("mode", "plan"), ("model", "legacy-two")] {
            let pick = select(&initial, category, value);
            let result = app
                .set_session_config(change(&id, pick.clone()))
                .await
                .unwrap();
            assert!(result.runtime.configuration.confirms(&pick));
        }
        // IDs are opaque per live connection; source-independent structure and
        // values are identical after replacing those arbitrary identities.
        let mut comparable = initial;
        for (index, option) in comparable.options.iter_mut().enumerate() {
            option.id = index.to_string();
        }
        configurations.push(comparable);
        app.shutdown().await.unwrap();
        store.close().await;
    }
    assert_eq!(configurations[0], configurations[1]);
}

#[tokio::test]
async fn grok_metadata_exposes_model_and_effort_without_inventing_permission_modes() {
    let (_dir, store, app, id) = setup("grok").await;
    let initial = configuration(&app, &id).await;
    let model = |value: &str| select(&initial, "model", value);
    let effort = |value: &str| select(&initial, "thought_level", value);
    assert!(initial.confirms(&model("grok-a")));
    assert!(initial.confirms(&effort("low")));
    assert!(
        !initial
            .options
            .iter()
            .any(|option| option.category.as_deref() == Some("mode"))
    );
    assert!(!serde_json::to_string(&initial).unwrap().contains("x.ai/"));
    let high = effort("high");
    assert!(
        app.set_session_config(change(&id, high.clone()))
            .await
            .unwrap()
            .runtime
            .configuration
            .confirms(&high)
    );
    let switched = app
        .set_session_config(change(&id, model("grok-b")))
        .await
        .unwrap();
    assert_eq!(switched.runtime.configuration.options.len(), 1);
    assert!(!switched.runtime.configuration.allows(&high));
    let switched = app
        .set_session_config(change(&id, model("grok-c")))
        .await
        .unwrap();
    assert!(switched.runtime.configuration.confirms(&effort("xhigh")));
    assert!(switched.runtime.configuration.allows(&effort("xhigh")));
    assert!(
        app.set_session_config(change(&id, model("denied")))
            .await
            .is_err()
    );
    assert!(configuration(&app, &id).await.confirms(&model("grok-c")));
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn grok_flat_metadata_is_supported_but_standard_options_take_precedence() {
    for fixture in ["grok-flat", "grok-native"] {
        let (_dir, store, app, id) = setup(fixture).await;
        let initial = configuration(&app, &id).await;
        if fixture == "grok-flat" {
            assert!(initial.confirms(&select(&initial, "thought_level", "low")));
            let high = select(&initial, "thought_level", "high");
            assert!(
                app.set_session_config(change(&id, high.clone()))
                    .await
                    .unwrap()
                    .runtime
                    .configuration
                    .confirms(&high)
            );
        } else {
            assert!(initial.confirms(&select(&initial, "model", "small")));
            assert!(!initial.allows(&select(&initial, "model", "grok-b")));
            assert_eq!(
                initial
                    .options
                    .iter()
                    .filter(|option| option.category.as_deref() == Some("model"))
                    .count(),
                1
            );
        }
        app.shutdown().await.unwrap();
        store.close().await;
    }
}

#[tokio::test]
async fn advertised_options_wait_for_ack_refresh_and_preserve_agent_refusal() {
    let (_dir, store, app, id) = setup("load").await;
    let initial = configuration(&app, &id).await;
    let model = |value: &str| select(&initial, "model", value);
    assert_eq!(initial.options.len(), 3);
    assert!(initial.confirms(&model("small")));
    assert!(
        !initial.allows(&model("legacy-two")),
        "standard category suppresses legacy model"
    );
    let SessionConfigKind::Select { options, .. } = &initial.options[0].kind else {
        panic!("select");
    };
    assert_eq!(options[0].group.as_deref(), Some("Family"));
    let task = tokio::spawn({
        let app = app.clone();
        let input = change(&id, model("large"));
        async move { app.set_session_config(input).await }
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!task.is_finished());
    assert!(configuration(&app, &id).await.confirms(&model("small")));
    let confirmed = task.await.unwrap().unwrap();
    assert!(confirmed.runtime.configuration.confirms(&model("large")));
    assert_eq!(
        confirmed.runtime.configuration.options.len(),
        4,
        "full response adds dependent option"
    );
    assert!(
        app.set_session_config(change(&id, model("denied")))
            .await
            .is_err()
    );
    assert!(configuration(&app, &id).await.confirms(&model("large")));
    let boolean = SessionConfigChange {
        config_id: initial
            .options
            .iter()
            .find(|option| option.name == "Toggle")
            .unwrap()
            .id
            .clone(),
        value: SessionConfigValue::Boolean { value: true },
    };
    assert!(
        app.set_session_config(change(&id, boolean.clone()))
            .await
            .unwrap()
            .runtime
            .configuration
            .confirms(&boolean)
    );
    let mode = select(&initial, "mode", "plan");
    assert!(
        app.set_session_config(change(&id, mode.clone()))
            .await
            .unwrap()
            .runtime
            .configuration
            .confirms(&mode)
    );
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn colliding_wire_identifiers_keep_distinct_stable_routes() {
    let (_dir, store, app, id) = setup("collisions").await;
    let initial = configuration(&app, &id).await;
    let ids: std::collections::HashSet<_> =
        initial.options.iter().map(|option| &option.id).collect();
    assert_eq!(ids.len(), 6);
    for option in &initial.options {
        let value = match option.category.as_deref() {
            Some("mode") => "plan",
            Some("model") => "legacy-two",
            _ => "b",
        };
        let pick = SessionConfigChange {
            config_id: option.id.clone(),
            value: SessionConfigValue::Select {
                value: value.into(),
            },
        };
        assert!(
            app.set_session_config(change(&id, pick.clone()))
                .await
                .unwrap()
                .runtime
                .configuration
                .confirms(&pick)
        );
    }
    assert_eq!(
        configuration(&app, &id)
            .await
            .options
            .iter()
            .map(|option| option.id.clone())
            .collect::<Vec<_>>(),
        initial
            .options
            .iter()
            .map(|option| option.id.clone())
            .collect::<Vec<_>>()
    );
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn explicit_mode_notifications_override_empty_ack_and_rpc_refusal_preserves_selection() {
    for fixture in ["mode-ack", "mode-refuse", "mode-rpc-refuse"] {
        let (_dir, store, app, id) = setup(fixture).await;
        let initial = configuration(&app, &id).await;
        let plan = select(&initial, "mode", "plan");
        let result = app.set_session_config(change(&id, plan.clone())).await;
        assert_eq!(result.is_ok(), fixture == "mode-ack");
        assert!(configuration(&app, &id).await.confirms(&select(
            &initial,
            "mode",
            if fixture == "mode-ack" { "plan" } else { "ask" }
        )));
        let denied = select(&initial, "model", "denied");
        assert!(app.set_session_config(change(&id, denied)).await.is_err());
        assert!(
            configuration(&app, &id)
                .await
                .confirms(&select(&initial, "model", "legacy-one"))
        );
        app.shutdown().await.unwrap();
        store.close().await;
    }
}

#[tokio::test]
async fn dynamic_standard_advertisements_replace_and_restore_legacy_routes() {
    let (_dir, store, app, id) = setup("load").await;
    let initial = configuration(&app, &id).await;
    let legacy = select(&initial, "mode", "plan");
    app.send_prompt(SendManagedPromptInput {
        session_id: id.session_id.clone(),
        text: "modern-mode".into(),
    })
    .await
    .unwrap();
    let modern = wait_for(&app, &id, |snapshot| {
        snapshot.runtime.activity == SessionActivityState::Idle
            && snapshot
                .runtime
                .configuration
                .options
                .iter()
                .any(|option| option.name == "Modern mode")
    })
    .await;
    let modern = select(&modern.runtime.configuration, "mode", "plan");
    assert_ne!(modern.config_id, legacy.config_id);
    assert!(!configuration(&app, &id).await.allows(&legacy));
    assert!(
        app.set_session_config(change(&id, modern.clone()))
            .await
            .unwrap()
            .runtime
            .configuration
            .confirms(&modern)
    );
    app.send_prompt(SendManagedPromptInput {
        session_id: id.session_id.clone(),
        text: "push".into(),
    })
    .await
    .unwrap();
    let restored = wait_for(&app, &id, |snapshot| {
        snapshot.runtime.activity == SessionActivityState::Idle
            && snapshot.runtime.configuration.allows(&legacy)
    })
    .await;
    assert_eq!(
        select(&restored.runtime.configuration, "mode", "plan").config_id,
        legacy.config_id
    );
    assert!(!restored.runtime.configuration.allows(&modern));
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn original_load_and_resume_restore_config_and_notifications_update_idle_snapshot() {
    for fixture in ["load", "resume"] {
        let (_dir, store, app, id) = setup(fixture).await;
        let original = app
            .get_session(id.clone())
            .await
            .unwrap()
            .session
            .management;
        let stale = select(&configuration(&app, &id).await, "model", "large");
        app.stop_session(id.clone()).await.unwrap();
        let loaded = app.start_session(id.clone()).await.unwrap();
        assert_eq!(loaded.session.management, original);
        let initial = loaded.runtime.configuration;
        assert!(matches!(
            app.set_session_config(change(&id, stale)).await,
            Err(SessionError::InvalidInput)
        ));
        let model = |value: &str| select(&initial, "model", value);
        assert!(initial.confirms(&model("large")));
        assert!(initial.confirms(&select(&initial, "mode", "plan")));
        app.send_prompt(SendManagedPromptInput {
            session_id: id.session_id.clone(),
            text: "push".into(),
        })
        .await
        .unwrap();
        let snapshot = wait_for(&app, &id, |snapshot| {
            snapshot.runtime.activity == SessionActivityState::Idle
                && snapshot.runtime.configuration.confirms(&model("small"))
        })
        .await;
        assert!(
            snapshot
                .runtime
                .configuration
                .confirms(&select(&initial, "mode", "ask"))
        );
        app.shutdown().await.unwrap();
        store.close().await;
    }
}

#[tokio::test]
async fn stop_interrupts_pending_setting_and_busy_or_unadvertised_changes_are_rejected() {
    let (_dir, store, app, id) = setup("load").await;
    let initial = configuration(&app, &id).await;
    app.send_prompt(SendManagedPromptInput {
        session_id: id.session_id.clone(),
        text: "wait".into(),
    })
    .await
    .unwrap();
    assert!(matches!(
        app.set_session_config(change(&id, select(&initial, "model", "large")))
            .await,
        Err(SessionError::Busy)
    ));
    app.stop_session(id.clone()).await.unwrap();
    let restarted = app.start_session(id.clone()).await.unwrap();
    let task = tokio::spawn({
        let app = app.clone();
        let input = change(
            &id,
            select(&restarted.runtime.configuration, "model", "hang"),
        );
        async move { app.set_session_config(input).await }
    });
    tokio::time::sleep(Duration::from_millis(40)).await;
    app.stop_session(id.clone()).await.unwrap();
    assert!(matches!(
        task.await.unwrap(),
        Err(SessionError::Interrupted)
    ));
    assert!(configuration(&app, &id).await.options.is_empty());
    app.shutdown().await.unwrap();
    store.close().await;
    let (_dir, store, app, id) = setup("none").await;
    assert!(matches!(
        app.set_session_config(change(&id, select(&initial, "model", "large")))
            .await,
        Err(SessionError::InvalidInput)
    ));
    app.shutdown().await.unwrap();
    store.close().await;
}
