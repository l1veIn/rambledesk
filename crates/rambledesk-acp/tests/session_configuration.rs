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
fn model(value: &str) -> SessionConfigChange {
    SessionConfigChange::Option {
        config_id: "model".into(),
        value: SessionConfigValue::Select {
            value: value.into(),
        },
    }
}

#[tokio::test]
async fn advertised_options_wait_for_ack_refresh_and_preserve_agent_refusal() {
    let (_dir, store, app, id) = setup("load").await;
    let initial = app
        .get_session(id.clone())
        .await
        .unwrap()
        .runtime
        .configuration;
    assert_eq!(initial.options.len(), 2);
    assert!(initial.confirms(&model("small")));
    let SessionConfigKind::Select { options, .. } = &initial.options[0].kind else {
        panic!("select")
    };
    assert_eq!(options[0].group.as_deref(), Some("Family"));
    let task = tokio::spawn({
        let app = app.clone();
        let input = change(&id, model("large"));
        async move { app.set_session_config(input).await }
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!task.is_finished());
    assert!(
        app.get_session(id.clone())
            .await
            .unwrap()
            .runtime
            .configuration
            .confirms(&model("small"))
    );
    let confirmed = task.await.unwrap().unwrap();
    assert!(confirmed.runtime.configuration.confirms(&model("large")));
    assert_eq!(
        confirmed.runtime.configuration.options.len(),
        3,
        "dependent options refresh from full response"
    );
    assert!(
        app.set_session_config(change(&id, model("denied")))
            .await
            .is_err()
    );
    assert!(
        app.get_session(id.clone())
            .await
            .unwrap()
            .runtime
            .configuration
            .confirms(&model("large"))
    );
    let boolean = SessionConfigChange::Option {
        config_id: "toggle".into(),
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
    for pick in [
        SessionConfigChange::Mode {
            mode_id: "plan".into(),
        },
        SessionConfigChange::Model {
            model_id: "legacy-two".into(),
        },
    ] {
        assert!(
            app.set_session_config(change(&id, pick.clone()))
                .await
                .unwrap()
                .runtime
                .configuration
                .confirms(&pick)
        );
    }
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn original_load_and_resume_restore_config_and_notifications_update_idle_snapshot() {
    for mode in ["load", "resume"] {
        let (_dir, store, app, id) = setup(mode).await;
        let original = app
            .get_session(id.clone())
            .await
            .unwrap()
            .session
            .management;
        app.stop_session(id.clone()).await.unwrap();
        let loaded = app.start_session(id.clone()).await.unwrap();
        assert_eq!(loaded.session.management, original);
        assert!(loaded.runtime.configuration.confirms(&model("large")));
        assert_eq!(
            loaded
                .runtime
                .configuration
                .modes
                .as_ref()
                .unwrap()
                .current_mode_id,
            "plan"
        );
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
        assert_eq!(
            snapshot
                .runtime
                .configuration
                .modes
                .unwrap()
                .current_mode_id,
            "ask"
        );
        app.shutdown().await.unwrap();
        store.close().await;
    }
}

#[tokio::test]
async fn stop_interrupts_pending_setting_and_busy_or_unadvertised_changes_are_rejected() {
    let (_dir, store, app, id) = setup("load").await;
    app.send_prompt(SendManagedPromptInput {
        session_id: id.session_id.clone(),
        text: "wait".into(),
    })
    .await
    .unwrap();
    assert!(matches!(
        app.set_session_config(change(&id, model("large"))).await,
        Err(SessionError::Busy)
    ));
    app.stop_session(id.clone()).await.unwrap();
    app.start_session(id.clone()).await.unwrap();
    let task = tokio::spawn({
        let app = app.clone();
        let input = change(&id, model("hang"));
        async move { app.set_session_config(input).await }
    });
    tokio::time::sleep(Duration::from_millis(40)).await;
    app.stop_session(id.clone()).await.unwrap();
    assert!(matches!(
        task.await.unwrap(),
        Err(SessionError::Interrupted)
    ));
    assert!(
        app.get_session(id)
            .await
            .unwrap()
            .runtime
            .configuration
            .options
            .is_empty()
    );
    app.shutdown().await.unwrap();
    store.close().await;

    let (_dir, store, app, id) = setup("none").await;
    assert!(matches!(
        app.set_session_config(change(&id, model("large"))).await,
        Err(SessionError::InvalidInput)
    ));
    app.shutdown().await.unwrap();
    store.close().await;
}
