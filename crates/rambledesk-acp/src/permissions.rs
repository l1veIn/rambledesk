use crate::AcpError;
use agent_client_protocol::{
    Responder,
    schema::v1::{
        PermissionOptionId, RequestPermissionOutcome, RequestPermissionRequest,
        RequestPermissionResponse, SelectedPermissionOutcome,
    },
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

struct PendingPermission {
    options: Vec<PermissionOptionId>,
    responder: Responder<RequestPermissionResponse>,
}

enum PendingRequest {
    Permission(PendingPermission),
    Input(crate::user_input::InputSpec, Responder<serde_json::Value>),
}

#[derive(Default)]
pub(crate) struct PermissionQueue(Mutex<HashMap<String, PendingRequest>>);

impl PermissionQueue {
    pub fn respond_interaction(
        &self,
        id: &str,
        response: rambledesk_core::SessionInteractionResponse,
    ) -> Result<(), AcpError> {
        use rambledesk_core::{SessionInputKind, SessionInteractionResponse};
        match response {
            SessionInteractionResponse::Permission { option_id } => {
                if !matches!(
                    self.0.lock().expect("permission queue lock").get(id),
                    Some(PendingRequest::Permission(_))
                ) {
                    return Err(AcpError::InvalidPermission);
                }
                self.respond(id, option_id.as_deref())
            }
            SessionInteractionResponse::Question { response } => {
                self.respond_typed_input(id, SessionInputKind::Question, response)
            }
            SessionInteractionResponse::Plan { response } => {
                self.respond_typed_input(id, SessionInputKind::Plan, response)
            }
        }
    }

    fn respond_typed_input(
        &self,
        id: &str,
        kind: rambledesk_core::SessionInputKind,
        response: rambledesk_core::SessionInputResponse,
    ) -> Result<(), AcpError> {
        if !matches!(self.0.lock().expect("permission queue lock").get(id), Some(PendingRequest::Input(spec, _)) if spec.kind == kind)
        {
            return Err(AcpError::InvalidPermission);
        }
        self.respond_input(id, response)
    }
    pub fn insert(
        &self,
        request: &RequestPermissionRequest,
        responder: Responder<RequestPermissionResponse>,
    ) -> String {
        let id = uuid::Uuid::now_v7().to_string();
        self.0.lock().expect("permission queue lock").insert(
            id.clone(),
            PendingRequest::Permission(PendingPermission {
                options: request
                    .options
                    .iter()
                    .map(|option| option.option_id.clone())
                    .collect(),
                responder,
            }),
        );
        id
    }
    pub fn insert_input(
        &self,
        spec: crate::user_input::InputSpec,
        responder: Responder<serde_json::Value>,
    ) -> String {
        let id = uuid::Uuid::now_v7().to_string();
        self.0
            .lock()
            .expect("permission queue lock")
            .insert(id.clone(), PendingRequest::Input(spec, responder));
        id
    }
    pub fn respond_input(
        &self,
        id: &str,
        response: rambledesk_core::SessionInputResponse,
    ) -> Result<(), AcpError> {
        let mut queue = self.0.lock().expect("permission queue lock");
        let Some(PendingRequest::Input(spec, _)) = queue.get(id) else {
            return Err(AcpError::InvalidPermission);
        };
        let answer = spec.answer(&response)?;
        let Some(PendingRequest::Input(_, responder)) = queue.remove(id) else {
            return Err(AcpError::InvalidPermission);
        };
        drop(queue);
        responder.respond(answer).map_err(|_| AcpError::Closed)
    }
    pub fn respond(&self, id: &str, option: Option<&str>) -> Result<(), AcpError> {
        let mut queue = self.0.lock().expect("permission queue lock");
        let pending = queue.get(id).ok_or(AcpError::InvalidPermission)?;
        if let PendingRequest::Input(spec, _) = pending {
            if option.is_some() {
                return Err(AcpError::InvalidPermission);
            }
            let answer = spec.cancel(rambledesk_core::SessionInputAction::Cancel);
            let Some(PendingRequest::Input(_, responder)) = queue.remove(id) else {
                return Err(AcpError::InvalidPermission);
            };
            drop(queue);
            return responder.respond(answer).map_err(|_| AcpError::Closed);
        }
        let PendingRequest::Permission(pending) = pending else {
            return Err(AcpError::InvalidPermission);
        };
        if option.is_some_and(|id| {
            !pending
                .options
                .iter()
                .any(|option| option.to_string() == id)
        }) {
            return Err(AcpError::InvalidPermission);
        }
        let Some(PendingRequest::Permission(pending)) = queue.remove(id) else {
            return Err(AcpError::InvalidPermission);
        };
        drop(queue);
        let outcome = match option {
            Some(option) => RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                PermissionOptionId::new(option),
            )),
            None => RequestPermissionOutcome::Cancelled,
        };
        pending
            .responder
            .respond(RequestPermissionResponse::new(outcome))
            .map_err(|_| AcpError::Closed)
    }
    pub fn cancel_all(&self) {
        let pending = std::mem::take(&mut *self.0.lock().expect("permission queue lock"));
        for (_, permission) in pending {
            match permission {
                PendingRequest::Permission(permission) => {
                    let _ = permission.responder.respond(RequestPermissionResponse::new(
                        RequestPermissionOutcome::Cancelled,
                    ));
                }
                PendingRequest::Input(spec, responder) => {
                    let _ =
                        responder.respond(spec.cancel(rambledesk_core::SessionInputAction::Cancel));
                }
            }
        }
    }
}

pub(crate) struct CancelPermissionsOnDrop(pub Arc<PermissionQueue>);
impl Drop for CancelPermissionsOnDrop {
    fn drop(&mut self) {
        self.0.cancel_all();
    }
}
