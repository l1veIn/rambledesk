use std::sync::Arc;

use crate::WakeupAdapter;

/// Host-specific wakeup adapters that can safely resume an existing agent turn.
///
/// RambleDesk currently exposes no generic CLI-resume adapters here: Claude
/// Code, Codex, and OpenCode can be poked from a separate process, but that is
/// not the same product guarantee as waking the original host context. Those
/// hosts intentionally fall back to the generic MCP adapter prompt.
///
/// Pi's native path is different: its package owns `request` + `wait` inside
/// the active Pi tool call, so there is no post-submit wakeup adapter to
/// register in this router.
pub fn known_host_wakeup_adapters() -> Vec<Arc<dyn WakeupAdapter>> {
    Vec::new()
}
