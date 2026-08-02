use std::sync::Arc;

use crate::{ContinuationStrategy, NativeWaitContinuationStrategy};

/// Host-specific continuation strategies that can safely resume an existing host turn.
///
/// RambleDesk currently exposes no generic CLI-resume strategies here: Claude
/// Code, Codex, and OpenCode can be poked from a separate process, but that is
/// not the same product guarantee as resuming the original host context. Those
/// hosts intentionally fall back to the generic MCP adapter prompt.
///
/// Pi's native path is different: its package owns `request` + `wait` inside
/// the active Pi tool call, so the router records that continuation is not required.
pub fn known_continuation_strategies() -> Vec<Arc<dyn ContinuationStrategy>> {
    vec![Arc::new(NativeWaitContinuationStrategy)]
}
