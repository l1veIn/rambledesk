use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum ApplicationResourceKey {
    All,
    Navigation,
    AgentConfigurations,
    ManagedSession {
        session_id: String,
    },
    HostSessionResources {
        host_id: String,
        host_session_id: String,
    },
    FeedbackWorkspace {
        request_id: String,
    },
    PublishedFeedback {
        request_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationChange {
    pub resources: Vec<ApplicationResourceKey>,
}

pub trait ApplicationChangeObserver: Send + Sync {
    fn observe(&self, change: ApplicationChange);
}

#[derive(Debug, Default)]
pub struct NoopApplicationChangeObserver;

impl ApplicationChangeObserver for NoopApplicationChangeObserver {
    fn observe(&self, _change: ApplicationChange) {}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ApplicationSnapshotMetadata {
    pub runtime_generation: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationSnapshot<Value> {
    pub metadata: ApplicationSnapshotMetadata,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationSnapshotError<Error> {
    Query(Error),
    Unstable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ApplicationInvalidation {
    pub runtime_generation: String,
    pub revision: String,
    pub resources: Vec<ApplicationResourceKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum ApplicationEvent {
    Ready {
        runtime_generation: String,
        revision: String,
    },
    Invalidate {
        runtime_generation: String,
        revision: String,
        resources: Vec<ApplicationResourceKey>,
    },
}

impl From<ApplicationInvalidation> for ApplicationEvent {
    fn from(value: ApplicationInvalidation) -> Self {
        Self::Invalidate {
            runtime_generation: value.runtime_generation,
            revision: value.revision,
            resources: value.resources,
        }
    }
}

#[derive(Debug)]
struct ApplicationChangeHubState {
    revision: u64,
    sender: broadcast::Sender<ApplicationInvalidation>,
}

/// Backend Runtime-owned invalidation ledger. Revision assignment and broadcast
/// are serialized so subscribers observe strictly increasing revisions.
#[derive(Debug, Clone)]
pub struct ApplicationChangeHub {
    runtime_generation: Arc<str>,
    state: Arc<Mutex<ApplicationChangeHubState>>,
}

impl Default for ApplicationChangeHub {
    fn default() -> Self {
        Self::new()
    }
}

impl ApplicationChangeHub {
    const CAPACITY: usize = 256;

    pub fn new() -> Self {
        Self::with_runtime_generation(Uuid::now_v7().to_string())
    }

    pub fn with_runtime_generation(runtime_generation: impl Into<String>) -> Self {
        let (sender, _) = broadcast::channel(Self::CAPACITY);
        Self {
            runtime_generation: Arc::from(runtime_generation.into()),
            state: Arc::new(Mutex::new(ApplicationChangeHubState {
                revision: 0,
                sender,
            })),
        }
    }

    pub fn metadata(&self) -> ApplicationSnapshotMetadata {
        let state = self.state.lock().expect("application change hub lock");
        ApplicationSnapshotMetadata {
            runtime_generation: self.runtime_generation.to_string(),
            revision: state.revision.to_string(),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ApplicationInvalidation> {
        self.state
            .lock()
            .expect("application change hub lock")
            .sender
            .subscribe()
    }

    pub fn subscribe_with_ready(
        &self,
    ) -> (
        ApplicationEvent,
        broadcast::Receiver<ApplicationInvalidation>,
    ) {
        let state = self.state.lock().expect("application change hub lock");
        let receiver = state.sender.subscribe();
        let ready = ApplicationEvent::Ready {
            runtime_generation: self.runtime_generation.to_string(),
            revision: state.revision.to_string(),
        };
        (ready, receiver)
    }

    pub async fn capture_snapshot<Value, Error, Query, QueryFuture>(
        &self,
        mut query: Query,
    ) -> Result<ApplicationSnapshot<Value>, ApplicationSnapshotError<Error>>
    where
        Query: FnMut() -> QueryFuture,
        QueryFuture: std::future::Future<Output = Result<Value, Error>>,
    {
        const MAX_ATTEMPTS: usize = 3;
        for _ in 0..MAX_ATTEMPTS {
            let before = self.metadata();
            let result = query().await;
            let after = self.metadata();
            if before == after {
                return result
                    .map(|value| ApplicationSnapshot {
                        metadata: before,
                        value,
                    })
                    .map_err(ApplicationSnapshotError::Query);
            }
        }
        Err(ApplicationSnapshotError::Unstable)
    }
}

impl ApplicationChangeObserver for ApplicationChangeHub {
    fn observe(&self, change: ApplicationChange) {
        if change.resources.is_empty() {
            return;
        }
        let mut state = self.state.lock().expect("application change hub lock");
        state.revision = state
            .revision
            .checked_add(1)
            .expect("application change revision overflow");
        let event = ApplicationInvalidation {
            runtime_generation: self.runtime_generation.to_string(),
            revision: state.revision.to_string(),
            resources: change.resources,
        };
        let _ = state.sender.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revisions_are_wire_safe_decimal_strings() {
        let hub = ApplicationChangeHub::with_runtime_generation("runtime-a");
        let mut receiver = hub.subscribe();

        hub.observe(ApplicationChange {
            resources: vec![ApplicationResourceKey::Navigation],
        });

        let event = receiver.try_recv().expect("invalidation");
        assert_eq!(event.runtime_generation, "runtime-a");
        assert_eq!(event.revision, "1");
        assert_eq!(hub.metadata().revision, "1");
    }

    #[tokio::test]
    async fn concurrent_notifications_are_strictly_monotonic() {
        let hub = ApplicationChangeHub::with_runtime_generation("runtime-a");
        let mut receiver = hub.subscribe();
        let tasks = (0..32)
            .map(|_| {
                let hub = hub.clone();
                tokio::spawn(async move {
                    hub.observe(ApplicationChange {
                        resources: vec![ApplicationResourceKey::Navigation],
                    });
                })
            })
            .collect::<Vec<_>>();
        for task in tasks {
            task.await.expect("notification task");
        }

        for expected in 1..=32 {
            let event = receiver.recv().await.expect("invalidation");
            assert_eq!(event.revision, expected.to_string());
        }
    }

    #[test]
    fn empty_changes_do_not_advance_the_revision() {
        let hub = ApplicationChangeHub::with_runtime_generation("runtime-a");
        hub.observe(ApplicationChange { resources: vec![] });
        assert_eq!(hub.metadata().revision, "0");
    }

    #[test]
    fn subscribe_with_ready_has_no_notification_gap() {
        let hub = ApplicationChangeHub::with_runtime_generation("runtime-a");
        let (ready, mut receiver) = hub.subscribe_with_ready();
        hub.observe(ApplicationChange {
            resources: vec![ApplicationResourceKey::Navigation],
        });

        assert!(matches!(ready, ApplicationEvent::Ready { revision, .. } if revision == "0"));
        assert_eq!(
            receiver
                .try_recv()
                .expect("post-ready invalidation")
                .revision,
            "1"
        );
    }

    #[tokio::test]
    async fn snapshot_capture_retries_if_the_projection_races_a_change() {
        let hub = ApplicationChangeHub::with_runtime_generation("runtime-a");
        let attempts = std::sync::atomic::AtomicUsize::new(0);
        let snapshot = hub
            .capture_snapshot(|| {
                let attempt = attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let hub = hub.clone();
                async move {
                    if attempt == 0 {
                        hub.observe(ApplicationChange {
                            resources: vec![ApplicationResourceKey::Navigation],
                        });
                    }
                    Ok::<_, ()>(format!("projection-{attempt}"))
                }
            })
            .await
            .expect("stable snapshot");
        assert_eq!(snapshot.metadata.revision, "1");
        assert_eq!(snapshot.value, "projection-1");
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
}
