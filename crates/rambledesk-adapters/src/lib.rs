//! Host wakeup / continuation adapters for RambleDesk.
//!
//! This crate owns the **control plane** that runs after a feedback request
//! reaches a terminal state. It intentionally lives outside `rambledesk-core`
//! so host-specific resume integrations can ship on a different cadence than
//! domain/protocol changes.
//!
//! - Missing or unknown host id → [`GenericWakeupAdapter`] (UI prompt path)
//! - Native host integrations should own their full request/get/wait path when
//!   the host can suspend inside that tool call (for example the Pi package).
//!   They do not need a post-submit wakeup adapter.

mod hosts;
mod presentation;
mod wakeup;

pub use hosts::known_host_wakeup_adapters;
pub use presentation::{AdapterPresentation, adapter_presentation, known_adapter_presentations};
pub use wakeup::{
    GenericWakeupAdapter, ResumePrompt, WakePayload, WakeReason, WakeResult, WakeupAdapter,
    WakeupRouter,
};
