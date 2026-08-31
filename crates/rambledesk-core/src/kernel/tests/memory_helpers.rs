use std::sync::PoisonError;

use super::{AcpSessionLinkSnapshot, MemoryFactStore, MemoryState, SessionRecord};

impl MemoryFactStore {
    pub(crate) fn inspect<T>(&self, reader: impl FnOnce(&MemoryState) -> T) -> T {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        reader(&state)
    }

    pub(crate) fn links(&self) -> Vec<AcpSessionLinkSnapshot> {
        self.inspect(|state| state.links.clone())
    }

    pub(crate) fn insert_session(&self, session: SessionRecord) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .sessions
            .insert(session.session_id.clone(), session);
    }
}
