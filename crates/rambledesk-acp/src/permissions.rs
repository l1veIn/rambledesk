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

#[derive(Default)]
pub(crate) struct PermissionQueue(Mutex<HashMap<String, PendingPermission>>);

impl PermissionQueue {
    pub fn insert(
        &self,
        request: &RequestPermissionRequest,
        responder: Responder<RequestPermissionResponse>,
    ) -> String {
        let id = uuid::Uuid::now_v7().to_string();
        self.0.lock().expect("permission queue lock").insert(
            id.clone(),
            PendingPermission {
                options: request
                    .options
                    .iter()
                    .map(|option| option.option_id.clone())
                    .collect(),
                responder,
            },
        );
        id
    }
    pub fn respond(&self, id: &str, option: Option<&str>) -> Result<(), AcpError> {
        let mut queue = self.0.lock().expect("permission queue lock");
        let pending = queue.get(id).ok_or(AcpError::InvalidPermission)?;
        if option.is_some_and(|id| {
            !pending
                .options
                .iter()
                .any(|option| option.to_string() == id)
        }) {
            return Err(AcpError::InvalidPermission);
        }
        let pending = queue.remove(id).ok_or(AcpError::InvalidPermission)?;
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
            let _ = permission.responder.respond(RequestPermissionResponse::new(
                RequestPermissionOutcome::Cancelled,
            ));
        }
    }
}

pub(crate) struct CancelPermissionsOnDrop(pub Arc<PermissionQueue>);
impl Drop for CancelPermissionsOnDrop {
    fn drop(&mut self) {
        self.0.cancel_all();
    }
}
