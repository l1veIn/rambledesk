use super::*;
use crate::{
    ApplicationChange, ApplicationChangeObserver, ApplicationResourceKey, Clock, IdGenerator,
    NoopApplicationChangeObserver, SystemClock, UuidV7Generator,
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use thiserror::Error;
use tokio::sync::{Mutex, watch};

#[derive(Debug, Error)]
pub enum SessionError {
    #[error(transparent)]
    Repository(#[from] SessionRepositoryError),
    #[error(transparent)]
    Driver(#[from] AgentDriverError),
    #[error("managed session is busy")]
    Busy,
    #[error("managed session is not connected")]
    NotConnected,
    #[error("managed session operation was interrupted")]
    Interrupted,
    #[error("session management is shutting down")]
    ShuttingDown,
    #[error("operation requires a managed session")]
    NotManaged,
    #[error("session input is invalid")]
    InvalidInput,
}

pub(super) struct LiveSession {
    pub runtime: SessionRuntime,
    pub connection: Option<Arc<dyn AgentSessionConnection>>,
    pub permissions: Vec<SessionPermission>,
    pub cancelling: bool,
}

pub(super) struct SessionEntry {
    pub live: Mutex<LiveSession>,
    pub lifecycle: Mutex<()>,
    pub interrupt: watch::Sender<u64>,
    pub events: Mutex<super::prompts::StreamState>,
}

impl Default for SessionEntry {
    fn default() -> Self {
        Self {
            live: Mutex::new(LiveSession {
                runtime: SessionRuntime::default(),
                connection: None,
                permissions: vec![],
                cancelling: false,
            }),
            lifecycle: Mutex::new(()),
            interrupt: watch::channel(0).0,
            events: Mutex::new(super::prompts::StreamState::default()),
        }
    }
}

#[derive(Clone)]
pub struct SessionApplication {
    pub(super) repository: Arc<dyn SessionRepository>,
    pub(super) activities: Arc<dyn SessionActivityRepository>,
    driver: Arc<dyn AgentSessionDriver>,
    pub(super) entries: Arc<Mutex<HashMap<String, Arc<SessionEntry>>>>,
    pub(super) clock: Arc<dyn Clock>,
    pub(super) ids: Arc<dyn IdGenerator>,
    pub(super) observer: Arc<dyn ApplicationChangeObserver>,
    closing: Arc<AtomicBool>,
}

impl SessionApplication {
    pub fn new(
        repository: Arc<dyn SessionRepository>,
        activities: Arc<dyn SessionActivityRepository>,
        driver: Arc<dyn AgentSessionDriver>,
    ) -> Self {
        Self {
            repository,
            activities,
            driver,
            entries: Arc::new(Mutex::new(HashMap::new())),
            clock: Arc::new(SystemClock),
            ids: Arc::new(UuidV7Generator),
            observer: Arc::new(NoopApplicationChangeObserver),
            closing: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_change_observer(mut self, observer: Arc<dyn ApplicationChangeObserver>) -> Self {
        self.observer = observer;
        self
    }

    pub async fn list_agent_configs(&self) -> Result<Vec<AgentConfig>, SessionError> {
        Ok(self.repository.list_agent_configs().await?)
    }

    pub async fn save_agent_config(
        &self,
        input: SaveAgentConfigInput,
    ) -> Result<AgentConfig, SessionError> {
        let now = self.clock.now_rfc3339();
        let (id, created_at) = match input.id {
            Some(id) => {
                let old = self.repository.get_agent_config(&id).await?;
                (id, old.created_at)
            }
            None => (self.ids.new_id(), now.clone()),
        };
        let saved = self
            .repository
            .save_agent_config(AgentConfig {
                id,
                name: input.name,
                host_id: input.host_id,
                protocol: input.protocol,
                enabled: input.enabled,
                command: input.command,
                args: input.args,
                env: input.env,
                created_at,
                updated_at: now,
            })
            .await?;
        self.changed();
        Ok(saved)
    }

    pub async fn delete_agent_config(&self, input: AgentConfigInput) -> Result<(), SessionError> {
        self.repository
            .delete_agent_config(&input.agent_config_id)
            .await?;
        self.changed();
        Ok(())
    }

    pub async fn check_agent_config(
        &self,
        input: AgentConfigInput,
    ) -> Result<AgentConnectionCheck, SessionError> {
        let config = self
            .repository
            .get_agent_config(&input.agent_config_id)
            .await?;
        let result = self.driver.check(&config).await;
        Ok(match result {
            Ok(caps) => AgentConnectionCheck {
                ok: true,
                message: "ACP connection succeeded".into(),
                details: vec![format!(
                    "Load: {}; resume: {}; HTTP MCP: {}",
                    caps.load_session, caps.resume_session, caps.http_mcp
                )],
            },
            Err(error) => AgentConnectionCheck {
                ok: false,
                message: error.message,
                details: vec![],
            },
        })
    }

    pub async fn create_session(
        &self,
        input: CreateManagedSessionInput,
    ) -> Result<ManagedSessionSnapshot, SessionError> {
        if self.closing.load(Ordering::SeqCst) {
            return Err(SessionError::ShuttingDown);
        }
        let session = self
            .repository
            .create_managed_session(NewManagedSession {
                session_id: self.ids.new_id(),
                agent_config_id: input.agent_config_id,
                cwd: input.cwd,
                title: input.title,
                created_at: self.clock.now_rfc3339(),
            })
            .await?;
        self.changed();
        // Creation is durable even if startup fails; the user can repair/retry it.
        let _ = self
            .start_session(ManagedSessionInput {
                session_id: session.session_id.clone(),
            })
            .await;
        self.get_session(ManagedSessionInput {
            session_id: session.session_id,
        })
        .await
    }

    pub async fn get_session(
        &self,
        input: ManagedSessionInput,
    ) -> Result<ManagedSessionSnapshot, SessionError> {
        let session = self.managed_record(&input.session_id).await?;
        let entry = self.entry(&input.session_id).await;
        let mut live = entry.live.lock().await;
        if live
            .connection
            .as_ref()
            .is_some_and(|connection| connection.is_closed())
        {
            live.runtime.connection = SessionConnectionState::Disconnected;
            live.permissions.clear();
            live.runtime.activity = SessionActivityState::Idle;
            live.runtime.last_error =
                Some("Agent connection closed; resume the original session to continue".into());
        }
        let runtime = live.runtime.clone();
        let permissions = live.permissions.clone();
        drop(live);
        let activities = self
            .activities
            .list_recent_session_activity(&input.session_id, 1000)
            .await?;
        Ok(ManagedSessionSnapshot {
            session,
            runtime,
            activities,
            permissions,
        })
    }

    pub async fn start_session(
        &self,
        input: ManagedSessionInput,
    ) -> Result<ManagedSessionSnapshot, SessionError> {
        if self.closing.load(Ordering::SeqCst) {
            return Err(SessionError::ShuttingDown);
        }
        let session = self.managed_record(&input.session_id).await?;
        let SessionManagement::Managed {
            agent_config_id, ..
        } = &session.management
        else {
            return Err(SessionError::NotManaged);
        };
        let config = self.repository.get_agent_config(agent_config_id).await?;
        if !config.enabled {
            return Err(SessionRepositoryError::AgentConfigDisabled.into());
        }
        let entry = self.entry(&input.session_id).await;
        let mut interrupted = entry.interrupt.subscribe();
        let _lifecycle = entry.lifecycle.lock().await;
        if self.closing.load(Ordering::SeqCst) {
            return Err(SessionError::ShuttingDown);
        }
        let mut live = entry.live.lock().await;
        if live
            .connection
            .as_ref()
            .is_some_and(|connection| !connection.is_closed())
        {
            drop(live);
            return self.get_session(input).await;
        }
        let old = live.connection.take();
        live.runtime.connection = SessionConnectionState::Connecting;
        let instance_id = self.ids.new_id();
        live.runtime.instance_id = Some(instance_id.clone());
        live.runtime.last_error = None;
        drop(live);
        if let Some(old) = old {
            old.stop().await?;
        }
        self.changed();
        let version = config.updated_at.clone();
        let result = tokio::select! {
            result=self.driver.start(AgentSessionLaunch{config,session,observer:Arc::new(super::prompts::SessionEventCollector{application:self.clone(),session_id:input.session_id.clone(),instance_id:instance_id.clone()})})=>result.map_err(SessionError::from),
            _=interrupted.changed()=>Err(SessionError::Interrupted),
        };
        let mut live = entry.live.lock().await;
        match result {
            Ok(started) => {
                if let Err(error) = self
                    .repository
                    .bind_remote_session(
                        &input.session_id,
                        &started.remote_session_id,
                        &self.clock.now_rfc3339(),
                    )
                    .await
                {
                    live.runtime.connection = SessionConnectionState::Failed;
                    live.runtime.last_error = Some(error.to_string());
                    drop(live);
                    let _ = started.connection.stop().await;
                    self.changed();
                    return Err(error.into());
                }
                live.runtime = SessionRuntime {
                    connection: SessionConnectionState::Connected,
                    activity: SessionActivityState::Idle,
                    instance_id: Some(instance_id),
                    config_updated_at: Some(version),
                    capabilities: started.capabilities,
                    last_error: None,
                };
                live.connection = Some(started.connection);
            }
            Err(error) => {
                live.runtime.connection = SessionConnectionState::Failed;
                live.runtime.last_error = Some(error.to_string());
                self.changed();
                return Err(error);
            }
        }
        drop(live);
        self.changed();
        self.get_session(input).await
    }

    pub async fn stop_session(
        &self,
        input: ManagedSessionInput,
    ) -> Result<ManagedSessionSnapshot, SessionError> {
        self.managed_record(&input.session_id).await?;
        let entry = self.entry(&input.session_id).await;
        entry
            .interrupt
            .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
        let _lifecycle = entry.lifecycle.lock().await;
        let mut live = entry.live.lock().await;
        let connection = live.connection.clone();
        live.runtime.connection = SessionConnectionState::Disconnected;
        live.permissions.clear();
        drop(live);
        if let Some(connection) = connection
            && let Err(error) = connection.stop().await
        {
            entry.live.lock().await.runtime.last_error = Some(error.to_string());
            self.changed();
            return Err(error.into());
        }
        let mut live = entry.live.lock().await;
        live.connection = None;
        live.runtime.connection = SessionConnectionState::Stopped;
        live.runtime.activity = SessionActivityState::Idle;
        live.runtime.instance_id = None;
        drop(live);
        self.changed();
        self.get_session(input).await
    }

    pub async fn shutdown(&self) -> Result<(), SessionError> {
        self.closing.store(true, Ordering::SeqCst);
        let entries = self.entries.lock().await.clone();
        for entry in entries.values() {
            entry
                .interrupt
                .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
        }
        let mut error = None;
        for session_id in entries.keys() {
            if let Err(failed) = self
                .stop_session(ManagedSessionInput {
                    session_id: session_id.clone(),
                })
                .await
            {
                error = Some(failed);
            }
        }
        match error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub(super) async fn managed_record(&self, id: &str) -> Result<SessionRecord, SessionError> {
        let record = self.repository.get_session(id).await?;
        if matches!(record.management, SessionManagement::External) {
            Err(SessionError::NotManaged)
        } else {
            Ok(record)
        }
    }
    pub(super) async fn entry(&self, id: &str) -> Arc<SessionEntry> {
        self.entries
            .lock()
            .await
            .entry(id.into())
            .or_default()
            .clone()
    }
    pub(super) fn changed(&self) {
        self.observer.observe(ApplicationChange {
            resources: vec![ApplicationResourceKey::All],
        });
    }
    pub(super) fn session_changed(&self, session_id: &str) {
        self.observer.observe(ApplicationChange {
            resources: vec![ApplicationResourceKey::ManagedSession {
                session_id: session_id.into(),
            }],
        });
    }
}
