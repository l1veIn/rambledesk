//! Host profiles, host knowledge and continuation strategies for RambleDesk.
//!
//! This crate owns the host knowledge registry (the single source of truth for
//! what a host is: identity, detection and Generic MCP install knowledge) and
//! post-terminal continuation strategy selection. It intentionally lives
//! outside `rambledesk-core` so host-specific integration metadata can ship on
//! a different cadence than application contract changes.
//!
//! - Missing or unknown host id -> [`ManualContinuationStrategy`] (UI prompt path)
//! - Native host integrations should own their full request/get/wait path when
//!   the host can suspend inside that tool call (for example the Pi package).
//!   They do not need a post-submit continuation strategy.

mod continuation;
mod hosts;
mod knowledge;
mod profile;

pub use continuation::{
    ContinuationPayload, ContinuationReason, ContinuationResult, ContinuationRouter,
    ContinuationStrategy, ManualContinuationStrategy, NativeWaitContinuationStrategy, ResumePrompt,
};
pub use hosts::known_continuation_strategies;
pub use knowledge::{ConfigFormat, HOSTS, HostKnowledge, generic_mcp_hosts};
pub use profile::{ContinuationMode, HostAdapter, HostProfile, host_profile, known_host_profiles};
