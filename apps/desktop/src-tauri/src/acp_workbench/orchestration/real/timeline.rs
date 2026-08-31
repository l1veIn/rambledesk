use rambledesk_acp_client::RunState;
use serde_json::Value;

use crate::acp_workbench::model::{
    SessionTimeline, TimelineEntry, TimelineEntryKind, TimelineEntryStatus, TimelineTurn,
    TimelineTurnStatus,
};

use super::mapping::now_rfc3339;

pub(super) const MAX_TIMELINE_TURNS: usize = 12;
pub(super) const MAX_TIMELINE_ENTRIES_PER_TURN: usize = 96;
const MAX_TIMELINE_CONTENT_BYTES: usize = 16 * 1024;
const MAX_TIMELINE_TITLE_BYTES: usize = 240;

#[derive(Default)]
pub(super) struct TimelineProjection {
    turns: Vec<TimelineTurn>,
    next_id: u64,
}

impl TimelineProjection {
    pub(super) fn snapshot(&self, session_id: &str) -> SessionTimeline {
        SessionTimeline {
            session_id: session_id.to_owned(),
            live_only: true,
            turns: self.turns.clone(),
        }
    }

    pub(super) fn apply_state(
        &mut self,
        session_id: &str,
        state: RunState,
        disconnect_reason: Option<&str>,
    ) {
        let now = now_rfc3339();
        match state {
            RunState::Running => {
                if self.start_turn_if_needed(session_id, &now) {
                    self.push_entry(
                        session_id,
                        TimelineEntryKind::Status,
                        "Agent is working",
                        "The Agent started a new turn.",
                        TimelineEntryStatus::Running,
                        &now,
                    );
                }
            }
            RunState::WaitingForPermission | RunState::WaitingForQuestion => {
                self.start_turn_if_needed(session_id, &now);
            }
            RunState::Ready => self.finish_current_turn(
                session_id,
                TimelineTurnStatus::Completed,
                TimelineEntryKind::Status,
                "Turn completed",
                "The Agent finished this turn.",
                &now,
            ),
            RunState::Stopped => self.finish_current_turn(
                session_id,
                TimelineTurnStatus::Completed,
                TimelineEntryKind::Status,
                "Agent stopped",
                "The Agent stopped after this turn.",
                &now,
            ),
            RunState::Disconnected => self.finish_current_turn(
                session_id,
                TimelineTurnStatus::Failed,
                TimelineEntryKind::Error,
                "Agent disconnected",
                disconnect_reason.unwrap_or("The ACP connection closed before the turn completed."),
                &now,
            ),
        }
    }

    pub(super) fn apply_update(&mut self, session_id: &str, update: &Value) {
        let now = now_rfc3339();
        let Some(kind) = update_discriminator(update) else {
            return;
        };
        if !matches!(
            kind.as_str(),
            "agent_thought_chunk"
                | "agent_message_chunk"
                | "tool_call"
                | "tool_call_update"
                | "plan"
        ) {
            return;
        }
        self.start_turn_if_needed(session_id, &now);
        match kind.as_str() {
            "agent_thought_chunk" => {
                if let Some(text) = chunk_text(update) {
                    self.append_chunk(
                        session_id,
                        TimelineEntryKind::Thought,
                        "Thinking",
                        &text,
                        &now,
                    );
                }
            }
            "agent_message_chunk" => {
                if let Some(text) = chunk_text(update) {
                    self.append_chunk(
                        session_id,
                        TimelineEntryKind::Message,
                        "Agent message",
                        &text,
                        &now,
                    );
                }
            }
            "tool_call" | "tool_call_update" => self.merge_tool_call(session_id, update, &now),
            "plan" => self.merge_plan(session_id, update, &now),
            _ => {}
        }
    }

    pub(super) fn mark_waiting(
        &mut self,
        session_id: &str,
        live_request_id: &str,
        title: &str,
        content: &str,
    ) {
        let now = now_rfc3339();
        self.start_turn_if_needed(session_id, &now);
        let entry_id = format!("wait:{live_request_id}");
        if let Some(entry) = self.current_entry_mut(&entry_id) {
            entry.title = bounded(title, MAX_TIMELINE_TITLE_BYTES);
            entry.content = bounded(content, MAX_TIMELINE_CONTENT_BYTES);
            entry.status = TimelineEntryStatus::Waiting;
            return;
        }
        self.push_entry_with_id(
            entry_id,
            TimelineEntryKind::Status,
            title,
            content,
            TimelineEntryStatus::Waiting,
            &now,
        );
    }

    pub(super) fn resolve_waiting(&mut self, live_request_id: &str) {
        let entry_id = format!("wait:{live_request_id}");
        if let Some(entry) = self.current_entry_mut(&entry_id) {
            entry.status = TimelineEntryStatus::Completed;
        }
    }

    fn start_turn_if_needed(&mut self, session_id: &str, started_at: &str) -> bool {
        if self
            .turns
            .last()
            .is_some_and(|turn| turn.status == TimelineTurnStatus::Running)
        {
            return false;
        }
        let turn_id = self.scoped_id(session_id, "turn");
        self.turns.push(TimelineTurn {
            turn_id,
            status: TimelineTurnStatus::Running,
            started_at: started_at.to_owned(),
            completed_at: None,
            entries: Vec::new(),
        });
        if self.turns.len() > MAX_TIMELINE_TURNS {
            let overflow = self.turns.len() - MAX_TIMELINE_TURNS;
            self.turns.drain(..overflow);
        }
        true
    }

    fn finish_current_turn(
        &mut self,
        session_id: &str,
        status: TimelineTurnStatus,
        final_kind: TimelineEntryKind,
        title: &str,
        content: &str,
        completed_at: &str,
    ) {
        if !self
            .turns
            .last()
            .is_some_and(|turn| turn.status == TimelineTurnStatus::Running)
        {
            return;
        }
        let final_entry_status = match status {
            TimelineTurnStatus::Failed => TimelineEntryStatus::Failed,
            TimelineTurnStatus::Running => TimelineEntryStatus::Running,
            TimelineTurnStatus::Completed => TimelineEntryStatus::Completed,
        };
        let entry_id = self.scoped_id(session_id, "entry");
        let turn = self.turns.last_mut().expect("running turn exists");
        for entry in &mut turn.entries {
            if matches!(
                entry.status,
                TimelineEntryStatus::Running | TimelineEntryStatus::Waiting
            ) {
                entry.status = final_entry_status;
            }
        }
        turn.entries.push(TimelineEntry {
            id: entry_id,
            kind: final_kind,
            title: bounded(title, MAX_TIMELINE_TITLE_BYTES),
            content: bounded(content, MAX_TIMELINE_CONTENT_BYTES),
            status: final_entry_status,
            created_at: completed_at.to_owned(),
        });
        trim_entries(&mut turn.entries);
        turn.status = status;
        turn.completed_at = Some(completed_at.to_owned());
    }

    fn append_chunk(
        &mut self,
        session_id: &str,
        kind: TimelineEntryKind,
        title: &str,
        chunk: &str,
        created_at: &str,
    ) {
        if chunk.is_empty() {
            return;
        }
        if let Some(entry) = self
            .current_turn_mut()
            .and_then(|turn| turn.entries.last_mut())
            && entry.kind == kind
            && entry.status == TimelineEntryStatus::Running
            && entry.title == title
        {
            entry.content.push_str(chunk);
            truncate_in_place(&mut entry.content, MAX_TIMELINE_CONTENT_BYTES);
            return;
        }
        self.push_entry(
            session_id,
            kind,
            title,
            chunk,
            TimelineEntryStatus::Running,
            created_at,
        );
    }

    fn merge_tool_call(&mut self, session_id: &str, update: &Value, created_at: &str) {
        let tool_call_id = string_field(update, &["toolCallId", "tool_call_id", "id"])
            .unwrap_or_else(|| self.scoped_id(session_id, "tool-call"));
        let entry_id = format!("tool:{tool_call_id}");
        let title = string_field(update, &["title", "name", "kind"]);
        let content = tool_content(update);
        let status = update
            .get("status")
            .and_then(Value::as_str)
            .and_then(map_entry_status);

        if let Some(entry) = self.current_entry_mut(&entry_id) {
            if let Some(title) = title {
                entry.title = bounded(&title, MAX_TIMELINE_TITLE_BYTES);
            }
            if let Some(content) = content {
                entry.content = bounded(&content, MAX_TIMELINE_CONTENT_BYTES);
            }
            if let Some(status) = status {
                entry.status = status;
            }
            return;
        }
        self.push_entry_with_id(
            entry_id,
            TimelineEntryKind::Tool,
            title.as_deref().unwrap_or("Tool call"),
            content.as_deref().unwrap_or(""),
            status.unwrap_or(TimelineEntryStatus::Running),
            created_at,
        );
    }

    fn merge_plan(&mut self, _session_id: &str, update: &Value, created_at: &str) {
        let Some(turn_id) = self.current_turn_mut().map(|turn| turn.turn_id.clone()) else {
            return;
        };
        let entry_id = format!("{turn_id}:plan");
        let (content, status) = plan_content(update);
        if let Some(entry) = self.current_entry_mut(&entry_id) {
            entry.content = bounded(&content, MAX_TIMELINE_CONTENT_BYTES);
            entry.status = status;
            return;
        }
        self.push_entry_with_id(
            entry_id,
            TimelineEntryKind::Status,
            "Plan",
            &content,
            status,
            created_at,
        );
    }

    fn push_entry(
        &mut self,
        session_id: &str,
        kind: TimelineEntryKind,
        title: &str,
        content: &str,
        status: TimelineEntryStatus,
        created_at: &str,
    ) {
        let id = self.scoped_id(session_id, "entry");
        self.push_entry_with_id(id, kind, title, content, status, created_at);
    }

    fn push_entry_with_id(
        &mut self,
        id: String,
        kind: TimelineEntryKind,
        title: &str,
        content: &str,
        status: TimelineEntryStatus,
        created_at: &str,
    ) {
        let Some(turn) = self.current_turn_mut() else {
            return;
        };
        turn.entries.push(TimelineEntry {
            id,
            kind,
            title: bounded(title, MAX_TIMELINE_TITLE_BYTES),
            content: bounded(content, MAX_TIMELINE_CONTENT_BYTES),
            status,
            created_at: created_at.to_owned(),
        });
        trim_entries(&mut turn.entries);
    }

    fn current_turn_mut(&mut self) -> Option<&mut TimelineTurn> {
        self.turns
            .last_mut()
            .filter(|turn| turn.status == TimelineTurnStatus::Running)
    }

    fn current_entry_mut(&mut self, id: &str) -> Option<&mut TimelineEntry> {
        self.current_turn_mut()?
            .entries
            .iter_mut()
            .find(|entry| entry.id == id)
    }

    fn scoped_id(&mut self, session_id: &str, kind: &str) -> String {
        self.next_id = self.next_id.saturating_add(1);
        format!("{session_id}:{kind}:{}", self.next_id)
    }
}

fn update_discriminator(update: &Value) -> Option<String> {
    string_field(update, &["sessionUpdate", "session_update", "type"])
        .map(|kind| kind.to_ascii_lowercase().replace('-', "_"))
}

fn chunk_text(update: &Value) -> Option<String> {
    update
        .get("content")
        .and_then(content_text)
        .or_else(|| update.get("text").and_then(content_text))
        .filter(|text| !text.is_empty())
}

fn tool_content(update: &Value) -> Option<String> {
    [
        "content",
        "rawOutput",
        "raw_output",
        "rawInput",
        "raw_input",
    ]
    .iter()
    .find_map(|key| update.get(*key).and_then(content_text))
    .filter(|text| !text.is_empty())
}

fn content_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(content_text)
                .collect::<Vec<_>>()
                .join("\n");
            (!text.is_empty()).then_some(text)
        }
        Value::Object(object) => {
            for key in ["text", "content", "message", "output"] {
                if let Some(text) = object.get(key).and_then(content_text)
                    && !text.is_empty()
                {
                    return Some(text);
                }
            }
            serde_json::to_string(value).ok()
        }
    }
}

fn plan_content(update: &Value) -> (String, TimelineEntryStatus) {
    let Some(entries) = update.get("entries").and_then(Value::as_array) else {
        return (
            content_text(update.get("content").unwrap_or(&Value::Null)).unwrap_or_default(),
            TimelineEntryStatus::Running,
        );
    };
    let mut aggregate = TimelineEntryStatus::Completed;
    let lines = entries
        .iter()
        .filter_map(|entry| {
            let status = entry
                .get("status")
                .and_then(Value::as_str)
                .and_then(map_entry_status)
                .unwrap_or(TimelineEntryStatus::Waiting);
            aggregate = merge_status(aggregate, status);
            let content = string_field(entry, &["content", "title", "description"])
                .or_else(|| entry.get("content").and_then(content_text))?;
            let status_label = entry
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("pending");
            Some(format!("- [{status_label}] {content}"))
        })
        .collect::<Vec<_>>();
    (lines.join("\n"), aggregate)
}

fn merge_status(current: TimelineEntryStatus, next: TimelineEntryStatus) -> TimelineEntryStatus {
    use TimelineEntryStatus::{Completed, Failed, Running, Waiting};
    match (current, next) {
        (Failed, _) | (_, Failed) => Failed,
        (Running, _) | (_, Running) => Running,
        (Waiting, _) | (_, Waiting) => Waiting,
        _ => Completed,
    }
}

fn map_entry_status(status: &str) -> Option<TimelineEntryStatus> {
    match status.to_ascii_lowercase().replace('-', "_").as_str() {
        "pending" | "queued" | "waiting" => Some(TimelineEntryStatus::Waiting),
        "in_progress" | "running" | "started" => Some(TimelineEntryStatus::Running),
        "completed" | "complete" | "success" | "succeeded" => Some(TimelineEntryStatus::Completed),
        "failed" | "error" | "cancelled" | "canceled" => Some(TimelineEntryStatus::Failed),
        _ => None,
    }
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .map(ToOwned::to_owned)
    })
}

fn trim_entries(entries: &mut Vec<TimelineEntry>) {
    if entries.len() > MAX_TIMELINE_ENTRIES_PER_TURN {
        let overflow = entries.len() - MAX_TIMELINE_ENTRIES_PER_TURN;
        entries.drain(..overflow);
    }
}

fn bounded(value: &str, max_bytes: usize) -> String {
    let mut value = value.to_owned();
    truncate_in_place(&mut value, max_bytes);
    value
}

fn truncate_in_place(value: &mut String, max_bytes: usize) {
    if value.len() <= max_bytes {
        return;
    }
    let mut boundary = max_bytes;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
}

#[cfg(test)]
mod tests;
