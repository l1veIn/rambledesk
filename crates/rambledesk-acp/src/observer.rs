use crate::{AcpError, AcpEvent};
use agent_client_protocol::schema::v1::{ContentBlock, SessionUpdate};
use async_trait::async_trait;
use rambledesk_core::{AgentSessionEvent, AgentSessionObserver, SessionActivityKind};
use std::sync::{Arc, Mutex};

#[async_trait]
pub(crate) trait ProtocolObserver: Send + Sync {
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
}

#[async_trait]
impl ProtocolObserver for ManagedObserver {
    async fn observe(&self, event: AcpEvent) -> Result<(), AcpError> {
        let update = match event {
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
        let (kind, text, tool_call_id, append) = match update.update {
            SessionUpdate::AgentMessageChunk(chunk) => match chunk.content {
                ContentBlock::Text(text) => {
                    (SessionActivityKind::AgentMessage, text.text, None, true)
                }
                _ => (
                    SessionActivityKind::Status,
                    "Agent sent non-text content".into(),
                    None,
                    false,
                ),
            },
            SessionUpdate::AgentThoughtChunk(chunk) => match chunk.content {
                ContentBlock::Text(text) => {
                    (SessionActivityKind::AgentThought, text.text, None, true)
                }
                _ => return Ok(()),
            },
            SessionUpdate::ToolCall(tool) => (
                SessionActivityKind::ToolCall,
                format!("{} · {:?}", tool.title, tool.status),
                Some(tool.tool_call_id.to_string()),
                true,
            ),
            SessionUpdate::ToolCallUpdate(tool) => (
                SessionActivityKind::ToolCall,
                format!(
                    "{}{}",
                    tool.fields.title.unwrap_or_default(),
                    tool.fields
                        .status
                        .map(|status| format!(" · {status:?}"))
                        .unwrap_or_else(|| "Updated".into())
                ),
                Some(tool.tool_call_id.to_string()),
                true,
            ),
            // User chunks echo input already persisted by the application. Other
            // capability/config notifications are not chat messages.
            _ => return Ok(()),
        };
        self.sink
            .observe(AgentSessionEvent::Activity {
                kind,
                text,
                tool_call_id,
                append,
            })
            .await
            .map_err(|_| AcpError::Protocol("activity persistence"))
    }
}
