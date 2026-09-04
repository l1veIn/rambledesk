use rambledesk_acp::{AcpConnection, AcpEvent, AcpLaunch};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

fn launch(mode: &str) -> AcpLaunch {
    AcpLaunch {
        command: "node".into(),
        args: vec![
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/agent.mjs")
                .to_string_lossy()
                .into_owned(),
            mode.into(),
        ],
        env: BTreeMap::new(),
        cwd: std::env::current_dir().unwrap(),
        mcp_servers: vec![],
    }
}

#[tokio::test]
async fn stdio_prompt_permission_cancel_and_original_context() {
    for mode in ["load", "resume", "none"] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let directory = tempfile::tempdir().unwrap();
        let closed = directory.path().join("closed");
        let mut launch = launch(mode);
        launch.env.insert(
            "FIXTURE_CLOSE_LOG".into(),
            closed.to_string_lossy().into_owned(),
        );
        let connection = AcpConnection::connect(
            &launch,
            Arc::new(move |event| sink.lock().unwrap().push(event)),
        )
        .await
        .unwrap();
        let info = connection.open_session(&launch, None).await.unwrap();
        assert_eq!(info.remote_session_id, "original-session");
        assert_eq!(
            connection
                .prompt(&info.remote_session_id, "hello")
                .await
                .unwrap(),
            "EndTurn"
        );
        assert!(
            events
                .lock()
                .unwrap()
                .iter()
                .any(|event| matches!(event, AcpEvent::Update(_)))
        );
        assert_eq!(
            connection
                .prompt(&info.remote_session_id, "permission")
                .await
                .unwrap(),
            "Cancelled"
        );
        let prompt = connection.prompt(&info.remote_session_id, "wait");
        let cancel = async {
            tokio::time::sleep(Duration::from_millis(30)).await;
            connection.cancel(&info.remote_session_id).unwrap();
        };
        let (result, _) = tokio::join!(prompt, cancel);
        assert_eq!(result.unwrap(), "Cancelled");
        connection.shutdown().await.unwrap();
        assert_eq!(
            std::fs::read_to_string(&closed).unwrap(),
            "original-session\n"
        );
        let connection = AcpConnection::connect(&launch, Arc::new(|_| {}))
            .await
            .unwrap();
        let resumed = connection
            .open_session(&launch, Some(&info.remote_session_id))
            .await;
        if mode == "none" {
            assert!(resumed.is_err());
        } else {
            assert_eq!(resumed.unwrap().remote_session_id, info.remote_session_id);
        }
        connection.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn invalid_launch_fails_before_spawning() {
    let mut launch = launch("none");
    launch.cwd = PathBuf::from("relative");
    assert!(
        AcpConnection::connect(&launch, Arc::new(|_| {}))
            .await
            .is_err()
    );
}
