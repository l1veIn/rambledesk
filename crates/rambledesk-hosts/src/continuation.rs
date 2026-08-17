//! Host continuation strategies for post-submit continuation.
//!
//! Specific hosts may later implement automatic turn resume. Until then, every
//! unmatched or missing host id falls through to [`ManualContinuationStrategy`], which
//! asks the human (via the RambleDesk UI) to return to the host chat and continue.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use rambledesk_core::FeedbackStatus;

use crate::{HostProfile, host_profile};

/// Terminal reason that should trigger a continuation decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationReason {
    Completed,
    Cancelled,
}

impl ContinuationReason {
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

/// Minimal facts needed to continue (or prompt) a host after a terminal event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinuationPayload {
    pub request_id: String,
    /// Install-time host id when known (`claude`, `codex`, …). Empty / unknown → generic.
    pub host_id: String,
    pub host_session_id: String,
    pub source_hint: Option<String>,
    pub reason: ContinuationReason,
}

impl ContinuationPayload {
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

/// What a continuation strategy decided to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContinuationResult {
    /// The host flow is already waiting for the terminal result.
    NotRequired {
        strategy_id: String,
        host_id: String,
    },
    /// A host-specific strategy delivered a continuation signal.
    HostDelivered {
        strategy_id: String,
        host_id: String,
    },
    /// No automatic continuation: show a prompt so the human resumes the host.
    UserPrompt {
        strategy_id: String,
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
    pub reason: ContinuationReason,
}

pub trait ContinuationStrategy: Send + Sync {
    fn id(&self) -> &'static str;

    fn profile(&self) -> HostProfile {
        host_profile(self.id())
    }

    /// Whether this strategy handles `host_id` (case-insensitive).
    fn matches_host(&self, host_id: &str) -> bool;

    fn continue_after_terminal(&self, payload: &ContinuationPayload) -> ContinuationResult;
}

/// Fallback when host is missing or no specific strategy matches.
#[derive(Debug, Default)]
pub struct ManualContinuationStrategy;

impl ManualContinuationStrategy {
    pub fn host_label(host_id: Option<&str>) -> String {
        host_profile(host_id.unwrap_or("generic")).label
    }

    pub fn build_prompt(payload: &ContinuationPayload) -> ResumePrompt {
        let host = payload.normalized_host_id();
        let host_label = Self::host_label(host);
        let host_id = host.unwrap_or("unknown").to_owned();
        let resume_prompt = payload.resume_prompt();
        let (title, body) = match payload.reason {
            ContinuationReason::Completed => (
                "反馈已提交 · 回到宿主点继续".to_owned(),
                format!(
                    "先回到 {host_label} 的对话，点等待中的「继续」或确认选项。现在的 skill 会用 ask / ask_user_question 卡住等你。只有宿主没有停下来等时，才需要粘贴下面的恢复提示。",
                ),
            ),
            ContinuationReason::Cancelled => (
                "反馈已取消 · 回到宿主点继续".to_owned(),
                format!(
                    "先回到 {host_label} 的对话，点等待中的确认以收尾。只有宿主没有停下来等时，才需要粘贴下面的提示并调用 get_feedback。",
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

impl ContinuationStrategy for ManualContinuationStrategy {
    fn id(&self) -> &'static str {
        "generic"
    }

    fn matches_host(&self, _host_id: &str) -> bool {
        false
    }

    fn continue_after_terminal(&self, payload: &ContinuationPayload) -> ContinuationResult {
        ContinuationResult::UserPrompt {
            strategy_id: self.id().to_owned(),
            prompt: Self::build_prompt(payload),
        }
    }
}

/// Native host flows that are already blocked in their own request call.
#[derive(Debug, Default)]
pub struct NativeWaitContinuationStrategy;

impl ContinuationStrategy for NativeWaitContinuationStrategy {
    fn id(&self) -> &'static str {
        "native"
    }

    fn matches_host(&self, host_id: &str) -> bool {
        // Pi and DeepSeek Harness own `request` + `wait` inside their active
        // tool call, so a terminal request needs no resume prompt at all.
        host_id.eq_ignore_ascii_case("pi") || host_id.eq_ignore_ascii_case("dsh")
    }

    fn continue_after_terminal(&self, payload: &ContinuationPayload) -> ContinuationResult {
        ContinuationResult::NotRequired {
            strategy_id: self.id().to_owned(),
            host_id: payload.host_id.clone(),
        }
    }
}

/// Resolves a host id to a specific continuation strategy or the generic fallback.
#[derive(Clone)]
pub struct ContinuationRouter {
    strategies: Arc<Vec<Arc<dyn ContinuationStrategy>>>,
    generic: Arc<dyn ContinuationStrategy>,
}

impl Default for ContinuationRouter {
    fn default() -> Self {
        Self::new(crate::known_continuation_strategies())
    }
}

impl std::fmt::Debug for ContinuationRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContinuationRouter")
            .field(
                "strategies",
                &self
                    .strategies
                    .iter()
                    .map(|strategy| strategy.id())
                    .collect::<Vec<_>>(),
            )
            .field("generic", &self.generic.id())
            .finish()
    }
}

impl ContinuationRouter {
    pub fn new(specific: Vec<Arc<dyn ContinuationStrategy>>) -> Self {
        Self {
            strategies: Arc::new(specific),
            generic: Arc::new(ManualContinuationStrategy),
        }
    }

    pub fn with_generic(
        specific: Vec<Arc<dyn ContinuationStrategy>>,
        generic: Arc<dyn ContinuationStrategy>,
    ) -> Self {
        Self {
            strategies: Arc::new(specific),
            generic,
        }
    }

    pub fn resolve(&self, host_id: Option<&str>) -> Arc<dyn ContinuationStrategy> {
        let Some(host_id) = host_id.map(str::trim).filter(|value| !value.is_empty()) else {
            return Arc::clone(&self.generic);
        };
        if host_id.eq_ignore_ascii_case("unknown") {
            return Arc::clone(&self.generic);
        }
        self.strategies
            .iter()
            .find(|strategy| strategy.matches_host(host_id))
            .cloned()
            .unwrap_or_else(|| Arc::clone(&self.generic))
    }

    pub fn continue_after_terminal(&self, payload: &ContinuationPayload) -> ContinuationResult {
        self.resolve(payload.normalized_host_id())
            .continue_after_terminal(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubStrategy;

    impl ContinuationStrategy for StubStrategy {
        fn id(&self) -> &'static str {
            "stub-claude"
        }

        fn matches_host(&self, host_id: &str) -> bool {
            host_id.eq_ignore_ascii_case("claude")
        }

        fn continue_after_terminal(&self, payload: &ContinuationPayload) -> ContinuationResult {
            ContinuationResult::HostDelivered {
                strategy_id: self.id().to_owned(),
                host_id: payload.host_id.clone(),
            }
        }
    }

    fn payload(host: &str) -> ContinuationPayload {
        ContinuationPayload {
            request_id: "0195f7e2-5c31-7b5a-8ab7-3c84ea4fc827".to_owned(),
            host_id: host.to_owned(),
            host_session_id: "session".to_owned(),
            source_hint: None,
            reason: ContinuationReason::Completed,
        }
    }

    #[test]
    fn missing_or_unknown_host_uses_generic_prompt() {
        let router = ContinuationRouter::new(vec![Arc::new(StubStrategy)]);
        for host in ["", "  ", "unknown", "codex"] {
            let result = router.continue_after_terminal(&payload(host));
            match result {
                ContinuationResult::UserPrompt {
                    strategy_id,
                    prompt,
                } => {
                    assert_eq!(strategy_id, "generic");
                    assert!(prompt.resume_prompt.contains(&payload(host).request_id));
                    assert!(prompt.resume_prompt.contains("get_feedback"));
                }
                other => panic!("expected user prompt, got {other:?}"),
            }
        }
    }

    #[test]
    fn matching_host_uses_specific_strategy() {
        let router = ContinuationRouter::new(vec![Arc::new(StubStrategy)]);
        let result = router.continue_after_terminal(&payload("claude"));
        assert_eq!(
            result,
            ContinuationResult::HostDelivered {
                strategy_id: "stub-claude".to_owned(),
                host_id: "claude".to_owned(),
            }
        );
    }

    #[test]
    fn generic_prompt_labels_known_hosts() {
        let prompt = ManualContinuationStrategy::build_prompt(&payload("codex"));
        assert_eq!(prompt.host_label, "Codex");
        assert_eq!(prompt.host_id, "codex");
        assert!(prompt.title.contains("继续"));
    }

    #[test]
    fn native_wait_does_not_create_a_resume_prompt() {
        let router = ContinuationRouter::new(vec![Arc::new(NativeWaitContinuationStrategy)]);
        assert_eq!(
            router.continue_after_terminal(&payload("pi")),
            ContinuationResult::NotRequired {
                strategy_id: "native".to_owned(),
                host_id: "pi".to_owned(),
            }
        );
        assert_eq!(
            router.continue_after_terminal(&payload("dsh")),
            ContinuationResult::NotRequired {
                strategy_id: "native".to_owned(),
                host_id: "dsh".to_owned(),
            }
        );
    }
}
