#[path = "../src/application_events.rs"]
mod application_events;

mod tests {
    use super::application_events::{ApplicationEventBridge, forward_events};
    use rambledesk_core::{
        ApplicationChange, ApplicationChangeHub, ApplicationChangeObserver, ApplicationEvent,
        ApplicationResourceKey,
    };
    use std::sync::Arc;
    use tauri::Listener;
    use tokio::{
        sync::mpsc,
        time::{Duration, timeout},
    };

    const APPLICATION_EVENTS_STREAM: &str = "rambledesk://application-events";

    fn invalidate(hub: &ApplicationChangeHub, session_id: &str) {
        hub.observe(ApplicationChange {
            resources: vec![ApplicationResourceKey::ManagedSession {
                session_id: session_id.into(),
            }],
        });
    }

    async fn next(receiver: &mut mpsc::UnboundedReceiver<ApplicationEvent>) -> ApplicationEvent {
        timeout(Duration::from_secs(2), receiver.recv())
            .await
            .expect("native event before timeout")
            .expect("native event stream")
    }

    #[tokio::test]
    async fn native_bridge_emits_every_managed_session_change_on_the_frontend_stream() {
        let app = tauri::test::mock_app();
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let listener = app.listen(APPLICATION_EVENTS_STREAM, move |event| {
            sender
                .send(serde_json::from_str(event.payload()).expect("application event"))
                .unwrap();
        });
        let hub = Arc::new(ApplicationChangeHub::with_runtime_generation(
            "desktop-runtime",
        ));
        let bridge = ApplicationEventBridge::start(app.handle().clone(), hub.clone());
        assert_eq!(
            next(&mut receiver).await,
            ApplicationEvent::Ready {
                runtime_generation: "desktop-runtime".into(),
                revision: "0".into(),
            }
        );

        for sequence in 1..=36 {
            let session_id = if sequence % 2 == 0 { "website" } else { "cli" };
            invalidate(&hub, session_id);
            assert_eq!(
                next(&mut receiver).await,
                ApplicationEvent::Invalidate {
                    runtime_generation: "desktop-runtime".into(),
                    revision: sequence.to_string(),
                    resources: vec![ApplicationResourceKey::ManagedSession {
                        session_id: session_id.into()
                    }],
                }
            );
        }
        bridge.cancel();
        app.unlisten(listener);
    }

    #[tokio::test]
    async fn lagged_native_receiver_requests_a_fresh_snapshot_then_continues_live_events() {
        let app = tauri::test::mock_app();
        let (sender, mut events) = mpsc::unbounded_channel();
        let listener = app.listen(APPLICATION_EVENTS_STREAM, move |event| {
            sender
                .send(serde_json::from_str(event.payload()).expect("application event"))
                .unwrap();
        });
        let hub = Arc::new(ApplicationChangeHub::with_runtime_generation(
            "desktop-runtime",
        ));
        let receiver = hub.subscribe();
        for _ in 0..512 {
            invalidate(&hub, "website");
        }
        let task = tokio::spawn(forward_events(app.handle().clone(), hub.clone(), receiver));
        assert_eq!(
            next(&mut events).await,
            ApplicationEvent::Ready {
                runtime_generation: "desktop-runtime".into(),
                revision: "512".into(),
            }
        );
        invalidate(&hub, "cli");
        assert_eq!(
            next(&mut events).await,
            ApplicationEvent::Invalidate {
                runtime_generation: "desktop-runtime".into(),
                revision: "513".into(),
                resources: vec![ApplicationResourceKey::ManagedSession {
                    session_id: "cli".into()
                }],
            }
        );
        task.abort();
        app.unlisten(listener);
    }
}
