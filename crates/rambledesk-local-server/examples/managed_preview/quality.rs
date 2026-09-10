//! Deterministic synthetic history persisted through the real activity repository.
use rambledesk_core::*;
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const TURN_COUNT: usize = 60;
pub const ACTIVITY_COUNT: usize = TURN_COUNT * 5;

#[derive(Serialize)]
pub struct HistorySeed {
    pub schema: u8,
    pub fixture: &'static str,
    pub scope: &'static str,
    pub session_id: String,
    pub session_title: String,
    pub turns: usize,
    pub seeded_activities: usize,
    pub stored_activities: usize,
    pub first_sequence: u64,
    pub last_sequence: u64,
    pub serialized_bytes: usize,
    pub stored_activity_sha256: String,
}

fn message(text: &str) -> Option<SessionActivityContent> {
    Some(SessionActivityContent::Message {
        blocks: vec![SessionContentBlock::Text { text: text.into() }],
        truncated: false,
    })
}

fn rows(session_id: &str) -> Vec<NewSessionActivity> {
    let mut rows = Vec::with_capacity(ACTIVITY_COUNT);
    for turn in 1..=TURN_COUNT {
        let mut tool = SessionToolCall::new(format!("quality-tool-{turn:03}"));
        tool.name = Some("fixture.read_file".into());
        tool.title = format!("Quality history tool {turn:03}");
        tool.kind = SessionToolKind::Read;
        tool.status = SessionToolStatus::Completed;
        tool.raw_input = Some(format!(
            r#"{{"path":"fixture/module-{turn:03}.ts","synthetic":true}}"#
        ));
        tool.raw_output = Some(format!(
            r#"{{"lines":24,"fixtureTurn":{turn},"synthetic":true}}"#
        ));
        tool.content = vec![SessionContentBlock::Text {
            text: format!(
                "Synthetic fixture output {turn:03}. No file was read and no model or tool was executed.\n\n{}",
                "Stable history detail for expansion and scrolling. ".repeat(12)
            ),
        }];
        let user = format!(
            "Quality history turn {turn:03}: inspect synthetic module {turn:03} and explain its feedback behavior."
        );
        let thought = format!(
            "Synthetic thought {turn:03}. {}",
            "Check document ownership, saved revisions, and explicit continuation. ".repeat(8)
        );
        let agent = format!(
            "### Quality history reply {turn:03}\n\nThis is a stored acceptance fixture, not model output.\n\n{}\n\n- Persist the draft before terminal actions.\n- Keep feedback and conversation identities explicit.\n\n```ts\nconst fixtureTurn = {turn};\n```",
            "Ordinary rendering content with **Markdown**, inline `code`, and 中文说明。 "
                .repeat(8)
        );
        let entries = [
            (
                SessionActivityKind::UserMessage,
                user.clone(),
                message(&user),
            ),
            (
                SessionActivityKind::AgentThought,
                thought.clone(),
                message(&thought),
            ),
            (
                SessionActivityKind::ToolCall,
                tool.title.clone(),
                Some(SessionActivityContent::ToolCall { tool }),
            ),
            (
                SessionActivityKind::AgentMessage,
                agent.clone(),
                message(&agent),
            ),
            (
                SessionActivityKind::Status,
                "Turn finished: end_turn (synthetic quality fixture)".into(),
                None,
            ),
        ];
        for (kind, text, content) in entries {
            let index = rows.len();
            rows.push(NewSessionActivity {
                id: format!("quality-{session_id}-{index:03}"),
                session_id: session_id.into(),
                turn_id: Some(format!("quality-turn-{turn:03}")),
                kind,
                text,
                content,
                tool_call_id: (kind == SessionActivityKind::ToolCall)
                    .then(|| format!("quality-tool-{turn:03}")),
                created_at: format!("2026-01-01T00:{:02}:{:02}.000Z", index / 60, index % 60),
            });
        }
    }
    rows
}

pub async fn seed(
    store: &dyn SessionActivityRepository,
    session_id: &str,
    session_title: &str,
) -> anyhow::Result<HistorySeed> {
    let mut seeded = Vec::with_capacity(ACTIVITY_COUNT);
    for row in rows(session_id) {
        seeded.push(store.append_activity(row).await?);
    }
    let stored = store.list_session_activity(session_id, None, 1_000).await?;
    anyhow::ensure!(
        seeded.len() == ACTIVITY_COUNT,
        "Incomplete quality history seed"
    );
    let serialized = serde_json::to_vec(&stored)?;
    Ok(HistorySeed {
        schema: 1,
        fixture: "rambledesk-managed-history-v1",
        scope: "Synthetic persisted display history, real SQLite/repository/application HTTP reads; local Node ACP fixture only, no model prompt or actual tool execution",
        session_id: session_id.into(),
        session_title: session_title.into(),
        turns: TURN_COUNT,
        seeded_activities: seeded.len(),
        stored_activities: stored.len(),
        first_sequence: seeded.first().unwrap().sequence,
        last_sequence: seeded.last().unwrap().sequence,
        serialized_bytes: serialized.len(),
        stored_activity_sha256: hex::encode(Sha256::digest(&serialized)),
    })
}
