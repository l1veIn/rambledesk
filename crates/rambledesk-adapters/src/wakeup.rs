//! Host wakeup adapters for post-submit continuation.
//!
//! Specific hosts may later implement automatic turn resume. Until then, every
//! unmatched or missing host id falls through to [`GenericWakeupAdapter`], which
//! asks the human (via the RambleDesk UI) to return to the host chat and continue.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use rambledesk_core::FeedbackStatus;

use crate::{AdapterPresentation, adapter_presentation};

/// Terminal reason that should attempt to resume the waiting agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WakeReason {
    Completed,
    Cancelled,
}

impl WakeReason {
    pub fn from_status(status: FeedbackStatus) -> Option<Self> {
        match status {
            FeedbackStatus::Completed => Some(Self::Completed),
            FeedbackStatus::Cancelled => Some(Self::Cancelled),
            FeedbackStatus::Waiting | FeedbackStatus::InProgress => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Minimal facts needed to wake (or prompt) a host after a terminal event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WakePayload {
    pub request_id: String,
    /// Install-time host id when known (`claude`, `codex`, …). Empty / unknown → generic.
    pub host_id: String,
    pub agent: String,
    pub session_id: String,
    /// Canonical project root for host CLIs that scope sessions by cwd.
    pub project_root_path: Option<String>,
    pub reason: WakeReason,
}

impl WakePayload {
    pub fn normalized_host_id(&self) -> Option<&str> {
        let candidate = self.host_id.trim();
        if candidate.is_empty() || candidate.eq_ignore_ascii_case("unknown") {
            return None;
        }
        Some(candidate)
    }

    pub fn resume_prompt(&self) -> String {
        format!(
            "RambleDesk feedback request {} is {}.\nCall get_feedback with this request_id, verify the package, and continue the original task.",
            self.request_id,
            self.reason.as_str()
        )
    }
}

/// What an adapter decided to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WakeResult {
    /// A host-specific adapter delivered a continuation signal.
    HostDelivered { adapter_id: String, host_id: String },
    /// No automatic wake: show a RambleDesk prompt so the human resumes the host.
    UserPrompt {
        adapter_id: String,
        prompt: ResumePrompt,
    },
}

/// UI payload for the generic (and any prompt-based) path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumePrompt {
    pub request_id: String,
    pub host_id: String,
    pub host_label: String,
    pub title: String,
    pub body: String,
    pub resume_prompt: String,
    pub reason: WakeReason,
}

pub trait WakeupAdapter: Send + Sync {
    fn id(&self) -> &'static str;

    fn presentation(&self) -> AdapterPresentation {
        adapter_presentation(self.id())
    }

    /// Whether this adapter owns `host_id` (case-insensitive).
    fn matches_host(&self, host_id: &str) -> bool;

    fn wake(&self, payload: &WakePayload) -> WakeResult;
}

/// Fallback when host is missing or no specific adapter matches.
#[derive(Debug, Default)]
pub struct GenericWakeupAdapter;

impl GenericWakeupAdapter {
    pub fn host_label(host_id: Option<&str>) -> String {
        adapter_presentation(host_id.unwrap_or("generic")).label
    }

    pub fn build_prompt(payload: &WakePayload) -> ResumePrompt {
        let host = payload.normalized_host_id();
        let host_label = Self::host_label(host);
        let host_id = host.unwrap_or("unknown").to_owned();
        let resume_prompt = payload.resume_prompt();
        let (title, body) = match payload.reason {
            WakeReason::Completed => (
                "反馈已提交 · 请回到 Agent 继续".to_owned(),
                format!(
                    "请切换到 {host_label} 的对话窗口，粘贴下面的恢复提示并发送（或直接说「继续」并调用 get_feedback）。无需手写 request_id。",
                ),
            ),
            WakeReason::Cancelled => (
                "反馈已取消 · 请回到 Agent 收尾".to_owned(),
                format!(
                    "请切换到 {host_label} 的对话窗口，粘贴下面的提示，让 Agent 用 get_feedback 确认取消并继续或收尾。",
                ),
            ),
        };
        ResumePrompt {
            request_id: payload.request_id.clone(),
            host_id,
            host_label,
            title,
            body,
            resume_prompt,
            reason: payload.reason,
        }
    }
}

impl WakeupAdapter for GenericWakeupAdapter {
    fn id(&self) -> &'static str {
        "generic"
    }

    fn matches_host(&self, _host_id: &str) -> bool {
        false
    }

    fn wake(&self, payload: &WakePayload) -> WakeResult {
        WakeResult::UserPrompt {
            adapter_id: self.id().to_owned(),
            prompt: Self::build_prompt(payload),
        }
    }
}

/// Resolves a host id to a specific adapter, otherwise the generic fallback.
#[derive(Clone)]
pub struct WakeupRouter {
    adapters: Arc<Vec<Arc<dyn WakeupAdapter>>>,
    generic: Arc<dyn WakeupAdapter>,
}

impl Default for WakeupRouter {
    fn default() -> Self {
        Self::new(crate::known_host_wakeup_adapters())
    }
}

impl std::fmt::Debug for WakeupRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WakeupRouter")
            .field(
                "adapters",
                &self
                    .adapters
                    .iter()
                    .map(|adapter| adapter.id())
                    .collect::<Vec<_>>(),
            )
            .field("generic", &self.generic.id())
            .finish()
    }
}

impl WakeupRouter {
    pub fn new(specific: Vec<Arc<dyn WakeupAdapter>>) -> Self {
        Self {
            adapters: Arc::new(specific),
            generic: Arc::new(GenericWakeupAdapter),
        }
    }

    pub fn with_generic(
        specific: Vec<Arc<dyn WakeupAdapter>>,
        generic: Arc<dyn WakeupAdapter>,
    ) -> Self {
        Self {
            adapters: Arc::new(specific),
            generic,
        }
    }

    pub fn resolve(&self, host_id: Option<&str>) -> Arc<dyn WakeupAdapter> {
        let Some(host_id) = host_id.map(str::trim).filter(|value| !value.is_empty()) else {
            return Arc::clone(&self.generic);
        };
        if host_id.eq_ignore_ascii_case("unknown") {
            return Arc::clone(&self.generic);
        }
        self.adapters
            .iter()
            .find(|adapter| adapter.matches_host(host_id))
            .cloned()
            .unwrap_or_else(|| Arc::clone(&self.generic))
    }

    pub fn wake(&self, payload: &WakePayload) -> WakeResult {
        self.resolve(payload.normalized_host_id()).wake(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubAdapter;

    impl WakeupAdapter for StubAdapter {
        fn id(&self) -> &'static str {
            "stub-claude"
        }

        fn matches_host(&self, host_id: &str) -> bool {
            host_id.eq_ignore_ascii_case("claude")
        }

        fn wake(&self, payload: &WakePayload) -> WakeResult {
            WakeResult::HostDelivered {
                adapter_id: self.id().to_owned(),
                host_id: payload.host_id.clone(),
            }
        }
    }

    fn payload(host: &str) -> WakePayload {
        WakePayload {
            request_id: "0195f7e2-5c31-7b5a-8ab7-3c84ea4fc827".to_owned(),
            host_id: host.to_owned(),
            agent: host.to_owned(),
            session_id: "session".to_owned(),
            project_root_path: None,
            reason: WakeReason::Completed,
        }
    }

    #[test]
    fn missing_or_unknown_host_uses_generic_prompt() {
        let router = WakeupRouter::new(vec![Arc::new(StubAdapter)]);
        for host in ["", "  ", "unknown", "codex"] {
            let result = router.wake(&payload(host));
            match result {
                WakeResult::UserPrompt { adapter_id, prompt } => {
                    assert_eq!(adapter_id, "generic");
                    assert!(prompt.resume_prompt.contains(&payload(host).request_id));
                    assert!(prompt.resume_prompt.contains("get_feedback"));
                }
                other => panic!("expected user prompt, got {other:?}"),
            }
        }
    }

    #[test]
    fn matching_host_uses_specific_adapter() {
        let router = WakeupRouter::new(vec![Arc::new(StubAdapter)]);
        let result = router.wake(&payload("claude"));
        assert_eq!(
            result,
            WakeResult::HostDelivered {
                adapter_id: "stub-claude".to_owned(),
                host_id: "claude".to_owned(),
            }
        );
    }

    #[test]
    fn generic_prompt_labels_known_hosts() {
        let prompt = GenericWakeupAdapter::build_prompt(&payload("codex"));
        assert_eq!(prompt.host_label, "Codex");
        assert_eq!(prompt.host_id, "codex");
        assert!(prompt.title.contains("继续"));
    }
}
