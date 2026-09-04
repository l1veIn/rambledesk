use std::sync::Arc;

use rambledesk_core::{ApplicationChangeHub, ApplicationEvent, ApplicationInvalidation};
use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::broadcast;

const APPLICATION_EVENTS_STREAM: &str = "rambledesk://application-events";

/// Keep the native transport subscribed for the lifetime of the desktop runtime.
pub(super) struct ApplicationEventBridge {
    task: tauri::async_runtime::JoinHandle<()>,
}

impl ApplicationEventBridge {
    pub(super) fn start<R: Runtime>(app: AppHandle<R>, hub: Arc<ApplicationChangeHub>) -> Self {
        // Subscribe before spawning so startup changes cannot fall between the
        // initial revision and the event receiver.
        let (ready, receiver) = hub.subscribe_with_ready();
        let task = tauri::async_runtime::spawn(async move {
            emit(&app, ready);
            forward_events(app, hub, receiver).await;
        });
        Self { task }
    }

    pub(super) fn cancel(&self) {
        self.task.abort();
    }
}

impl Drop for ApplicationEventBridge {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn emit<R: Runtime>(app: &AppHandle<R>, event: ApplicationEvent) {
    if let Err(error) = app.emit(APPLICATION_EVENTS_STREAM, event) {
        tracing::warn!(%error, "could not emit desktop application change");
    }
}

pub(super) async fn forward_events<R: Runtime>(
    app: AppHandle<R>,
    hub: Arc<ApplicationChangeHub>,
    mut receiver: broadcast::Receiver<ApplicationInvalidation>,
) {
    loop {
        match receiver.recv().await {
            Ok(invalidation) => emit(&app, invalidation.into()),
            Err(broadcast::error::RecvError::Lagged(_)) => {
                // A readiness event makes every mounted projection reread. A
                // fresh receiver avoids replaying invalidations below its revision.
                let (ready, next_receiver) = hub.subscribe_with_ready();
                receiver = next_receiver;
                emit(&app, ready);
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}
