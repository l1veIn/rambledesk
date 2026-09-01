use std::{
    fmt,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tokio::sync::watch;

const TOKEN_BYTES: usize = 32;
const DURABLE_TOKEN_HEX_LENGTH: usize = TOKEN_BYTES * 2;

#[derive(Clone, PartialEq, Eq)]
pub struct DurableWebAccessToken(String);

impl DurableWebAccessToken {
    pub fn generate() -> Self {
        let bytes: [u8; TOKEN_BYTES] = rand::random();
        Self(hex::encode(bytes))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, WebSessionError> {
        let value = value.into();
        if value.len() != DURABLE_TOKEN_HEX_LENGTH
            || !value.as_bytes().iter().all(u8::is_ascii_hexdigit)
        {
            return Err(WebSessionError::InvalidDurableToken);
        }
        Ok(Self(value.to_ascii_lowercase()))
    }

    pub fn secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for DurableWebAccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DurableWebAccessToken([REDACTED])")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebSessionPolicy {
    pub idle_timeout_seconds: u64,
    pub absolute_timeout_seconds: u64,
    pub max_sessions: usize,
}

impl Default for WebSessionPolicy {
    fn default() -> Self {
        Self {
            idle_timeout_seconds: 30 * 60,
            absolute_timeout_seconds: 12 * 60 * 60,
            max_sessions: 32,
        }
    }
}

pub trait WebSessionClock: Send + Sync {
    fn now_seconds(&self) -> u64;
}

struct SystemWebSessionClock;

impl WebSessionClock for SystemWebSessionClock {
    fn now_seconds(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

#[derive(Debug, Clone)]
struct SessionRecord {
    issued_at: u64,
    last_seen_at: u64,
    runtime_generation: String,
}

struct SessionEntry {
    token_hash: [u8; 32],
    record: SessionRecord,
}

struct WebSessionState {
    durable_token: DurableWebAccessToken,
    runtime_generation: String,
    sessions: Vec<SessionEntry>,
}

pub struct WebSessionManager {
    policy: WebSessionPolicy,
    clock: Arc<dyn WebSessionClock>,
    state: Mutex<WebSessionState>,
    revocation: watch::Sender<u64>,
}

impl WebSessionManager {
    pub fn new(
        durable_token: DurableWebAccessToken,
        runtime_generation: impl Into<String>,
    ) -> Self {
        Self::with_clock(
            durable_token,
            runtime_generation,
            WebSessionPolicy::default(),
            Arc::new(SystemWebSessionClock),
        )
    }

    pub fn with_policy(
        durable_token: DurableWebAccessToken,
        runtime_generation: impl Into<String>,
        policy: WebSessionPolicy,
    ) -> Self {
        Self::with_clock(
            durable_token,
            runtime_generation,
            policy,
            Arc::new(SystemWebSessionClock),
        )
    }

    pub fn issue_session(&self, durable_token: &str) -> Option<String> {
        let now = self.clock.now_seconds();
        let mut state = self.state.lock().expect("Web Session state poisoned");
        if !crate::web_security::constant_time_bytes_eq(
            state.durable_token.secret().as_bytes(),
            durable_token.as_bytes(),
        ) {
            return None;
        }
        purge_expired(&mut state, self.policy, now);
        if state.sessions.len() >= self.policy.max_sessions {
            return None;
        }

        let token_bytes: [u8; TOKEN_BYTES] = rand::random();
        let token = URL_SAFE_NO_PAD.encode(token_bytes);
        let token_hash = hash_token(&token);
        let runtime_generation = state.runtime_generation.clone();
        state.sessions.push(SessionEntry {
            token_hash,
            record: SessionRecord {
                issued_at: now,
                last_seen_at: now,
                runtime_generation,
            },
        });
        Some(token)
    }

    pub fn authorize(&self, session_token: &str) -> Option<WebSessionAuthorization> {
        let now = self.clock.now_seconds();
        let mut state = self.state.lock().expect("Web Session state poisoned");
        let runtime_generation = state.runtime_generation.clone();
        let token_hash = hash_token(session_token);
        let index = constant_time_session_index(&state.sessions, &token_hash)?;
        let record = &mut state.sessions[index].record;
        if session_expired(record, self.policy, now)
            || record.runtime_generation != runtime_generation
        {
            state.sessions.remove(index);
            return None;
        }
        record.last_seen_at = now;
        let expires_at = record
            .issued_at
            .saturating_add(self.policy.absolute_timeout_seconds)
            .min(now.saturating_add(self.policy.idle_timeout_seconds));
        let revocation = self.revocation.subscribe();
        let revocation_epoch = *revocation.borrow();
        Some(WebSessionAuthorization {
            expires_at,
            revocation_epoch,
            revocation,
            clock: self.clock.clone(),
        })
    }

    pub fn revoke_all(&self) {
        self.state
            .lock()
            .expect("Web Session state poisoned")
            .sessions
            .clear();
        self.revocation
            .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
    }

    pub fn rotate_durable_token(&self, durable_token: DurableWebAccessToken) {
        let mut state = self.state.lock().expect("Web Session state poisoned");
        state.durable_token = durable_token;
        state.sessions.clear();
        drop(state);
        self.revocation
            .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
    }

    pub fn bind_runtime_generation(&self, runtime_generation: impl Into<String>) {
        let runtime_generation = runtime_generation.into();
        let mut state = self.state.lock().expect("Web Session state poisoned");
        if state.runtime_generation == runtime_generation {
            return;
        }
        state.runtime_generation = runtime_generation;
        state.sessions.clear();
        drop(state);
        self.revocation
            .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
    }

    fn with_clock(
        durable_token: DurableWebAccessToken,
        runtime_generation: impl Into<String>,
        policy: WebSessionPolicy,
        clock: Arc<dyn WebSessionClock>,
    ) -> Self {
        assert!(policy.idle_timeout_seconds > 0);
        assert!(policy.absolute_timeout_seconds > 0);
        assert!(policy.max_sessions > 0);
        let (revocation, _) = watch::channel(0);
        Self {
            policy,
            clock,
            state: Mutex::new(WebSessionState {
                durable_token,
                runtime_generation: runtime_generation.into(),
                sessions: Vec::new(),
            }),
            revocation,
        }
    }
}

impl crate::WebSessionAuthenticator for WebSessionManager {
    fn authorize(&self, session_token: &str) -> Option<WebSessionAuthorization> {
        WebSessionManager::authorize(self, session_token)
    }
}

#[derive(Clone)]
pub struct WebSessionAuthorization {
    expires_at: u64,
    revocation_epoch: u64,
    revocation: watch::Receiver<u64>,
    clock: Arc<dyn WebSessionClock>,
}

impl WebSessionAuthorization {
    pub fn is_active(&self) -> bool {
        *self.revocation.borrow() == self.revocation_epoch
            && self.clock.now_seconds() < self.expires_at
    }

    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }

    pub async fn revoked(&mut self) {
        let epoch = self.revocation_epoch;
        loop {
            let now = self.clock.now_seconds();
            if *self.revocation.borrow() != epoch || now >= self.expires_at {
                return;
            }
            tokio::select! {
                result = self.revocation.changed() => {
                    if result.is_err() || *self.revocation.borrow() != epoch {
                        return;
                    }
                }
                () = tokio::time::sleep(Duration::from_secs(self.expires_at - now)) => return,
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WebSessionError {
    #[error("Web Access token must be exactly 64 hexadecimal characters")]
    InvalidDurableToken,
}

fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

fn constant_time_session_index(
    sessions: &[SessionEntry],
    candidate_hash: &[u8; 32],
) -> Option<usize> {
    let mut matched = None;
    for (index, session) in sessions.iter().enumerate() {
        if bool::from(session.token_hash.ct_eq(candidate_hash)) {
            matched = Some(index);
        }
    }
    matched
}

fn purge_expired(state: &mut WebSessionState, policy: WebSessionPolicy, now: u64) {
    let runtime_generation = &state.runtime_generation;
    state.sessions.retain(|session| {
        session.record.runtime_generation == *runtime_generation
            && !session_expired(&session.record, policy, now)
    });
}

fn session_expired(record: &SessionRecord, policy: WebSessionPolicy, now: u64) -> bool {
    now >= record
        .last_seen_at
        .saturating_add(policy.idle_timeout_seconds)
        || now
            >= record
                .issued_at
                .saturating_add(policy.absolute_timeout_seconds)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    struct FakeClock(AtomicU64);

    impl FakeClock {
        fn advance(&self, seconds: u64) {
            self.0.fetch_add(seconds, Ordering::SeqCst);
        }
    }

    impl WebSessionClock for FakeClock {
        fn now_seconds(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    fn manager(clock: Arc<FakeClock>) -> WebSessionManager {
        WebSessionManager::with_clock(
            DurableWebAccessToken::parse("a".repeat(64)).expect("durable token"),
            "runtime-a",
            WebSessionPolicy {
                idle_timeout_seconds: 30,
                absolute_timeout_seconds: 100,
                max_sessions: 2,
            },
            clock,
        )
    }

    #[test]
    fn durable_exchange_issues_only_short_memory_sessions() {
        let clock = Arc::new(FakeClock(AtomicU64::new(10)));
        let manager = manager(clock);
        assert!(manager.issue_session(&"b".repeat(64)).is_none());
        let token = manager
            .issue_session(&"a".repeat(64))
            .expect("short session");
        assert_ne!(token, "a".repeat(64));
        assert!(
            token
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        );
        assert!(manager.authorize(&token).is_some());
    }

    #[test]
    fn session_credentials_are_matched_by_full_constant_time_hash_scan() {
        let clock = Arc::new(FakeClock(AtomicU64::new(10)));
        let manager = manager(clock);
        let first = manager
            .issue_session(&"a".repeat(64))
            .expect("first session");
        let second = manager
            .issue_session(&"a".repeat(64))
            .expect("second session");

        assert!(manager.authorize(&first).is_some());
        assert!(manager.authorize(&second).is_some());
        assert!(manager.authorize("not-a-session-token").is_none());
    }

    #[test]
    fn idle_and_absolute_expiry_use_the_injected_clock() {
        let clock = Arc::new(FakeClock(AtomicU64::new(10)));
        let manager = manager(clock.clone());
        let idle = manager.issue_session(&"a".repeat(64)).expect("idle token");
        let authorization = manager.authorize(&idle).expect("active authorization");
        clock.advance(29);
        assert!(authorization.is_active());
        assert!(manager.authorize(&idle).is_some());
        clock.advance(30);
        assert!(!authorization.is_active());
        assert!(manager.authorize(&idle).is_none());

        let absolute = manager
            .issue_session(&"a".repeat(64))
            .expect("absolute token");
        for _ in 0..3 {
            clock.advance(25);
            assert!(manager.authorize(&absolute).is_some());
        }
        clock.advance(25);
        assert!(manager.authorize(&absolute).is_none());
    }

    #[test]
    fn stop_rotation_and_runtime_generation_revoke_existing_sessions() {
        let clock = Arc::new(FakeClock(AtomicU64::new(10)));
        let manager = manager(clock);

        let stopped = manager
            .issue_session(&"a".repeat(64))
            .expect("stopped token");
        manager.revoke_all();
        assert!(manager.authorize(&stopped).is_none());

        let rotated = manager
            .issue_session(&"a".repeat(64))
            .expect("rotated token");
        manager.rotate_durable_token(
            DurableWebAccessToken::parse("b".repeat(64)).expect("replacement token"),
        );
        assert!(manager.authorize(&rotated).is_none());
        assert!(manager.issue_session(&"a".repeat(64)).is_none());

        let generation = manager
            .issue_session(&"b".repeat(64))
            .expect("generation token");
        manager.bind_runtime_generation("runtime-b");
        assert!(manager.authorize(&generation).is_none());
    }
}
