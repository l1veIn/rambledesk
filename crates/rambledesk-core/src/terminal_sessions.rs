//! Request-scoped, bounded interactive PTYs shared by desktop and Web Access.
//! Handles live in the application, so changing the visible workbench does not
//! terminate the process. Input is deliberately never retained in a transcript.
mod buffer;
mod runtime;
#[cfg(test)]
mod tests;

use crate::ApplicationError;
use runtime::TerminalRuntime;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};
use ts_rs::TS;

pub const MAX_TERMINAL_OUTPUT_BYTES: usize = 256 * 1024;
pub const MAX_TERMINAL_INPUT_BYTES: usize = 16 * 1024;
const MAX_RUNNING_SESSIONS: usize = 16;
const MAX_REQUEST_SESSIONS: usize = 16;
const MAX_RETAINED_SESSIONS: usize = 64;
const MAX_CLOSED_REQUESTS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum TerminalSessionStatus {
    Running,
    Exited,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct OpenTerminalSessionInput {
    pub request_id: String,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TerminalSessionInput {
    pub request_id: String,
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReadTerminalSessionInput {
    pub request_id: String,
    pub session_id: String,
    #[serde(default)]
    #[ts(type = "number | null")]
    pub after_sequence: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WriteTerminalSessionInput {
    pub request_id: String,
    pub session_id: String,
    pub data: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ResizeTerminalSessionInput {
    pub request_id: String,
    pub session_id: String,
    pub cols: u16,
    pub rows: u16,
}

/// `output` retains ANSI sequences. Cursors count UTF-8 bytes of the decoded
/// stream. A null cursor replays the retained buffer; an older cursor returns
/// its retained tail with `first_sequence > after_sequence`, requiring a reset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct TerminalSessionSnapshot {
    pub session_id: String,
    pub request_id: String,
    pub cwd: String,
    pub shell: String,
    pub cols: u16,
    pub rows: u16,
    pub status: TerminalSessionStatus,
    pub exit_code: Option<i32>,
    pub output: String,
    #[ts(type = "number")]
    pub first_sequence: u64,
    #[ts(type = "number")]
    pub next_sequence: u64,
    pub truncated: bool,
}

#[derive(Default)]
struct Registry {
    sessions: HashMap<String, RegisteredSession>,
    order: VecDeque<String>,
    closed: VecDeque<String>,
}

struct RegisteredSession {
    runtime: Arc<TerminalRuntime>,
    // Retained history remains reclaimable after the bounded request tombstone
    // queue forgets this request. This flag is bounded by the session registry.
    request_closed: bool,
}

impl Registry {
    fn request_closed(&self, request_id: &str) -> bool {
        self.closed.iter().any(|id| id == request_id)
            || self
                .sessions
                .values()
                .any(|item| item.request_closed && item.runtime.request_id() == request_id)
    }

    fn evict_finished(&mut self) -> Result<(), ApplicationError> {
        let position = self
            .order
            .iter()
            .position(|id| {
                self.sessions
                    .get(id)
                    .is_some_and(|item| item.request_closed && item.runtime.is_finished())
            })
            .ok_or_else(|| unavailable("Terminal session capacity has been reached."))?;
        let removed = self.order.remove(position).expect("retained terminal");
        self.sessions.remove(&removed);
        Ok(())
    }
}

#[derive(Default)]
struct ManagerInner {
    registry: Mutex<Registry>,
    opening: Arc<tokio::sync::Mutex<()>>,
}

impl Drop for ManagerInner {
    fn drop(&mut self) {
        for session in self
            .registry
            .get_mut()
            .expect("terminal registry")
            .sessions
            .values()
        {
            session.runtime.request_stop();
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct TerminalSessionManager {
    inner: Arc<ManagerInner>,
}

impl TerminalSessionManager {
    pub async fn open(
        &self,
        request_id: String,
        cwd: String,
        shell: Option<String>,
        cols: u16,
        rows: u16,
    ) -> Result<TerminalSessionSnapshot, ApplicationError> {
        validate_size(cols, rows)?;
        let opening = self.inner.opening.clone().lock_owned().await;
        let inner = self.inner.clone();
        // Opening a PTY and launching its shell can block (notably ConPTY).
        tokio::task::spawn_blocking(move || {
            // A disconnected caller cannot cancel OS spawn. Keep serialization
            // owned by this task until the request-bound session is registered.
            let _opening = opening;
            let mut registry = inner.registry.lock().expect("terminal registry");
            if registry.request_closed(&request_id) {
                return Err(unavailable("The feedback request has ended."));
            }
            if let Some(session) = registry
                .sessions
                .values()
                .map(|session| &session.runtime)
                .find(|session| session.request_id() == request_id && session.is_running())
            {
                return session.snapshot(None);
            }
            let previous: Vec<_> = registry
                .sessions
                .values()
                .map(|session| &session.runtime)
                .filter(|session| session.request_id() == request_id)
                .collect();
            if previous.iter().any(|session| !session.is_finished()) {
                return Err(unavailable(
                    "The previous terminal session is still stopping; try again shortly.",
                ));
            }
            if previous.len() >= MAX_REQUEST_SESSIONS {
                return Err(unavailable(
                    "This request has reached its limit of 16 terminal sessions.",
                ));
            }
            let running = registry
                .sessions
                .values()
                .filter(|item| !item.runtime.is_finished())
                .count();
            if running >= MAX_RUNNING_SESSIONS {
                return Err(unavailable(
                    "Too many terminal sessions are running; stop one first.",
                ));
            }
            while registry.sessions.len() >= MAX_RETAINED_SESSIONS {
                registry.evict_finished()?;
            }
            drop(registry);
            let session = Arc::new(TerminalRuntime::spawn(
                &request_id,
                &cwd,
                shell.as_deref(),
                cols,
                rows,
            )?);
            let mut registry = inner.registry.lock().expect("terminal registry");
            if registry.request_closed(&request_id) {
                session.request_stop();
                return Err(unavailable("The feedback request has ended."));
            }
            let snapshot = session.snapshot(None)?;
            registry.order.push_back(session.id().to_owned());
            registry.sessions.insert(
                session.id().to_owned(),
                RegisteredSession {
                    runtime: session,
                    request_closed: false,
                },
            );
            Ok(snapshot)
        })
        .await
        .map_err(|_| unavailable("Terminal session could not be opened."))?
    }

    fn session(
        &self,
        request_id: &str,
        session_id: &str,
    ) -> Result<Arc<TerminalRuntime>, ApplicationError> {
        self.inner
            .registry
            .lock()
            .expect("terminal registry")
            .sessions
            .get(session_id)
            .filter(|item| item.runtime.request_id() == request_id)
            .map(|item| item.runtime.clone())
            .ok_or_else(|| unavailable("The terminal session was not found for this request."))
    }

    pub fn read(
        &self,
        input: &ReadTerminalSessionInput,
    ) -> Result<TerminalSessionSnapshot, ApplicationError> {
        self.session(&input.request_id, &input.session_id)?
            .snapshot(input.after_sequence)
    }

    pub async fn write(
        &self,
        input: WriteTerminalSessionInput,
    ) -> Result<TerminalSessionSnapshot, ApplicationError> {
        if input.data.is_empty() || input.data.len() > MAX_TERMINAL_INPUT_BYTES {
            return Err(unavailable(
                "Terminal input must contain between 1 and 16384 bytes.",
            ));
        }
        self.session(&input.request_id, &input.session_id)?
            .write(input.data)
            .await
    }

    pub async fn resize(
        &self,
        input: ResizeTerminalSessionInput,
    ) -> Result<TerminalSessionSnapshot, ApplicationError> {
        validate_size(input.cols, input.rows)?;
        self.session(&input.request_id, &input.session_id)?
            .resize(input.cols, input.rows)
            .await
    }

    pub async fn stop(
        &self,
        input: TerminalSessionInput,
    ) -> Result<TerminalSessionSnapshot, ApplicationError> {
        self.session(&input.request_id, &input.session_id)?
            .stop()
            .await
    }

    pub fn stop_request(&self, request_id: &str) {
        let mut registry = self.inner.registry.lock().expect("terminal registry");
        for session in registry
            .sessions
            .values_mut()
            .filter(|session| session.runtime.request_id() == request_id)
        {
            session.request_closed = true;
            session.runtime.request_stop();
        }
        if !registry.closed.iter().any(|id| id == request_id) {
            registry.closed.push_back(request_id.to_owned());
            if registry.closed.len() > MAX_CLOSED_REQUESTS {
                registry.closed.pop_front();
            }
        }
    }
}

pub(crate) fn validate_size(cols: u16, rows: u16) -> Result<(), ApplicationError> {
    if !(20..=300).contains(&cols) || !(5..=100).contains(&rows) {
        return Err(unavailable(
            "Terminal size must be 20–300 columns and 5–100 rows.",
        ));
    }
    Ok(())
}

fn unavailable(message: impl Into<String>) -> ApplicationError {
    ApplicationError::invalid_argument(message)
}
