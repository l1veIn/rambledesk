use crate::{AcpError, AcpEvent};
use async_trait::async_trait;
use rambledesk_core::{AgentSessionEvent, AgentSessionObserver, SessionActivityKind};
use std::sync::{Arc, Mutex};

#[async_trait]
pub(crate) trait ProtocolObserver: Send + Sync {
    fn manages_permissions(&self) -> bool {
        false
    }
    async fn observe(&self, event: AcpEvent) -> Result<(), AcpError>;
}

pub(crate) struct CallbackObserver(pub Arc<dyn Fn(AcpEvent) + Send + Sync>);
#[async_trait]
impl ProtocolObserver for CallbackObserver {
    async fn observe(&self, event: AcpEvent) -> Result<(), AcpError> {
        (self.0)(event);
        Ok(())
    }
}

pub(crate) struct ManagedObserver {
    pub sink: Arc<dyn AgentSessionObserver>,
    pub remote: Mutex<Option<String>>,
    pub local_session_id: String,
}

#[async_trait]
impl ProtocolObserver for ManagedObserver {
    fn manages_permissions(&self) -> bool {
        true
    }
    async fn observe(&self, event: AcpEvent) -> Result<(), AcpError> {
        let update = match event {
            AcpEvent::PermissionRequested {
                request_id,
                request,
            } => {
                let remote = self.remote.lock().expect("remote attribution lock").clone();
                if remote.as_deref() != Some(request.session_id.to_string().as_str()) {
                    return Err(AcpError::Protocol("permission attribution"));
                }
                let permission = rambledesk_core::SessionPermission {
                    request_id,
                    session_id: self.local_session_id.clone(),
                    details: crate::permission_details::describe(&request.tool_call.fields),
                    title: request
                        .tool_call
                        .fields
                        .title
                        .unwrap_or_else(|| "Agent tool operation".into()),
                    options: request
                        .options
                        .into_iter()
                        .map(|option| rambledesk_core::SessionPermissionOption {
                            option_id: option.option_id.to_string(),
                            name: option.name,
                            kind: format!("{:?}", option.kind),
                        })
                        .collect(),
                };
                return self
                    .sink
                    .observe(AgentSessionEvent::PermissionRequested(permission))
                    .await
                    .map_err(|_| AcpError::Protocol("permission event"));
            }
            AcpEvent::PermissionDeclined => {
                return self
                    .sink
                    .observe(AgentSessionEvent::Activity {
                        kind: SessionActivityKind::Status,
                        text: "Agent permission was declined".into(),
                        tool_call_id: None,
                        append: false,
                    })
                    .await
                    .map_err(|_| AcpError::Protocol("activity persistence"));
            }
            AcpEvent::Update(notification) => notification,
        };
        let remote = self.remote.lock().expect("remote attribution lock").clone();
        let Some(remote) = remote else {
            return Ok(());
        };
        if update.session_id.to_string() != remote {
            return Err(AcpError::Protocol("session attribution"));
        }
        let Some(event) = crate::activity_content::convert(update.update)? else {
            return Ok(());
        };
        self.sink
            .observe(event)
            .await
            .map_err(|_| AcpError::Protocol("activity persistence"))
    }
}
