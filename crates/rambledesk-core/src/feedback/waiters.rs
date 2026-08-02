use std::{
    collections::HashMap,
    sync::{Mutex, PoisonError},
};

use tokio::sync::watch;

#[derive(Default)]
pub(super) struct FeedbackWaiters {
    channels: Mutex<HashMap<String, watch::Sender<u64>>>,
}

impl FeedbackWaiters {
    pub(super) fn subscribe(&self, request_id: &str) -> watch::Receiver<u64> {
        let mut channels = self.channels.lock().unwrap_or_else(PoisonError::into_inner);
        channels
            .entry(request_id.to_owned())
            .or_insert_with(|| watch::channel(0).0)
            .subscribe()
    }

    pub(super) fn notify_terminal(&self, request_id: &str) {
        let sender = self
            .channels
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(request_id);
        if let Some(sender) = sender {
            sender.send_modify(|generation| *generation += 1);
        }
    }
}
