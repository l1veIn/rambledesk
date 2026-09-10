//! Shared validation and pending-input lifecycle. Wire formats belong to codecs.
use crate::{AcpError, AcpEvent};
use agent_client_protocol::{Agent, Builder, Client, HandleDispatchFrom, Responder};
use rambledesk_core::{
    SessionInputAction, SessionInputKind, SessionInputRequest, SessionInputResponse,
};
use serde_json::Value;
use std::sync::Arc;
mod cursor;
mod form;
mod grok;
mod schema;
use schema::{check_schema, validate};

type Observer = Arc<dyn crate::observer::ProtocolObserver>;
type Queue = Arc<crate::permissions::PermissionQueue>;

/// A fixed codec selected by the registered RPC method, never by payload text.
struct InputProtocol {
    kind: SessionInputKind,
    decode: fn(&Value, Option<&str>) -> Result<InputFields, AcpError>,
    accept: fn(&Value) -> Result<Value, AcpError>,
    cancel: fn(SessionInputAction) -> Value,
}

struct InputFields {
    remote: String,
    title: String,
    schema: Value,
}

pub(crate) struct InputSpec {
    protocol: &'static InputProtocol,
    pub kind: SessionInputKind,
    pub remote: String,
    pub title: String,
    pub schema: Value,
}

impl InputSpec {
    fn parse(
        protocol: &'static InputProtocol,
        raw: &Value,
        active_session: Option<&str>,
    ) -> Result<Self, AcpError> {
        let InputFields {
            remote,
            title,
            schema,
        } = (protocol.decode)(raw, active_session)?;
        if remote.is_empty()
            || title.len() > 65536
            || schema.to_string().len() > 262144
            || !schema.is_object()
        {
            return Err(AcpError::InvalidPermission);
        }
        Ok(Self {
            protocol,
            kind: protocol.kind,
            remote,
            title,
            schema,
        })
    }

    pub fn answer(&self, response: &SessionInputResponse) -> Result<Value, AcpError> {
        let content = response
            .content_json
            .as_deref()
            .map(|raw| {
                if raw.len() > 262144 {
                    return Err(AcpError::InvalidPermission);
                }
                serde_json::from_str::<Value>(raw).map_err(|_| AcpError::InvalidPermission)
            })
            .transpose()?;
        if response.action != SessionInputAction::Accept {
            if content.as_ref().is_some_and(|v| !v.is_null()) {
                return Err(AcpError::InvalidPermission);
            }
            return Ok(self.cancel(response.action));
        }
        let content = content.as_ref().ok_or(AcpError::InvalidPermission)?;
        check_schema(&self.schema, 0)?;
        validate(&self.schema, content)?;
        (self.protocol.accept)(content)
    }

    pub fn cancel(&self, action: SessionInputAction) -> Value {
        (self.protocol.cancel)(action)
    }
}

pub(crate) fn register<H: HandleDispatchFrom<Agent>>(
    builder: Builder<Client, H>,
    queue: Queue,
    observer: Observer,
) -> Builder<Client, impl HandleDispatchFrom<Agent>> {
    let builder = form::register(builder, queue.clone(), observer.clone());
    let builder = grok::register(builder, queue.clone(), observer.clone());
    cursor::register(builder, queue, observer)
}

async fn receive(
    protocol: &'static InputProtocol,
    raw: Value,
    responder: Responder<Value>,
    queue: &crate::permissions::PermissionQueue,
    observer: &Observer,
) -> Result<(), agent_client_protocol::Error> {
    let spec = match InputSpec::parse(protocol, &raw, observer.remote_session_id().as_deref()) {
        Ok(spec) => spec,
        Err(_) => {
            return responder
                .respond_with_internal_error("Unsupported or malformed agent input request");
        }
    };
    if !observer.manages_permissions() {
        return responder.respond(spec.cancel(SessionInputAction::Decline));
    }
    let kind = spec.kind;
    let remote = spec.remote.clone();
    let title = spec.title.clone();
    let mut visible_schema = spec.schema.clone();
    if check_schema(&spec.schema, 0).is_err() {
        visible_schema["x-rambledesk-unsupported"] = Value::Bool(true);
    }
    let input = SessionInputRequest {
        schema_json: visible_schema.to_string(),
    };
    let request_id = queue.insert_input(spec, responder);
    if observer
        .observe(AcpEvent::InputRequested {
            request_id: request_id.clone(),
            kind,
            remote,
            title,
            input,
        })
        .await
        .is_err()
    {
        let _ = queue.respond(&request_id, None);
        return Err(agent_client_protocol::Error::internal_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
