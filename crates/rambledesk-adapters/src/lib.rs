//! Host wakeup / continuation adapters for RambleDesk.
//!
//! This crate owns the **control plane** that runs after a feedback request
//! reaches a terminal state. It intentionally lives outside `rambledesk-core`
//! so host-specific resume integrations can ship on a different cadence than
//! domain/protocol changes.
//!
//! - Missing or unknown host id → [`GenericWakeupAdapter`] (UI prompt path)
//! - Matched host id → future specific adapters (Claude, Codex, …)

mod presentation;
mod wakeup;

pub use presentation::{AdapterPresentation, adapter_presentation, known_adapter_presentations};
pub use wakeup::{
    GenericWakeupAdapter, ResumePrompt, WakePayload, WakeReason, WakeResult, WakeupAdapter,
    WakeupRouter,
};
