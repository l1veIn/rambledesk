use super::*;

impl SessionApplication {
    pub(super) async fn record_message_chunk(
        &self,
        session_id: &str,
        stream: &mut StreamState,
        kind: SessionActivityKind,
        block: SessionContentBlock,
        truncated: bool,
    ) -> Result<(), SessionError> {
        if !matches!(
            kind,
            SessionActivityKind::AgentMessage | SessionActivityKind::AgentThought
        ) {
            return Err(SessionError::InvalidInput);
        }
        let existing = stream
            .last
            .as_ref()
            .filter(|row| row.kind == kind && row.content.is_some());
        let mut content = existing.and_then(|row| row.content.clone()).unwrap_or(
            SessionActivityContent::Message {
                blocks: vec![],
                truncated: false,
            },
        );
        if let SessionActivityContent::Message {
            blocks,
            truncated: was_truncated,
        } = &mut content
        {
            // Truncation can mean an omitted media payload. Preserve later text
            // while the shared message budget still has capacity.
            let bytes = blocks
                .iter()
                .map(SessionContentBlock::byte_len)
                .sum::<usize>();
            let available = MAX_ACTIVITY_TEXT_BYTES.saturating_sub(bytes);
            match block {
                SessionContentBlock::Text { mut text } => {
                    let original = text.len();
                    let mut end = original.min(available);
                    while !text.is_char_boundary(end) {
                        end -= 1;
                    }
                    text.truncate(end);
                    *was_truncated |= original != end;
                    if let Some(SessionContentBlock::Text { text: last }) = blocks.last_mut() {
                        last.push_str(&text);
                    } else if blocks.len() < MAX_ACTIVITY_CONTENT_BLOCKS {
                        blocks.push(SessionContentBlock::Text { text });
                    } else {
                        *was_truncated = true;
                    }
                }
                block
                    if blocks.len() < MAX_ACTIVITY_CONTENT_BLOCKS
                        && block.byte_len() <= available =>
                {
                    blocks.push(block)
                }
                _ => *was_truncated = true,
            }
            *was_truncated |= truncated;
        }
        let text = content.summary();
        let row = if let Some(row) = existing {
            self.activities
                .update_activity_content(&row.id, session_id, &text, &content)
                .await?
        } else {
            self.append_structured_activity(
                session_id,
                stream.turn_id.as_deref(),
                kind,
                content,
                None,
            )
            .await?
        };
        stream.last = Some(row);
        Ok(())
    }

    pub(super) async fn record_tool_patch(
        &self,
        session_id: &str,
        stream: &mut StreamState,
        tool_call_id: String,
        patch: SessionToolCallPatch,
    ) -> Result<(), SessionError> {
        let existing = stream.tools.get(&tool_call_id);
        let mut tool = match existing.and_then(|row| row.content.as_ref()) {
            Some(SessionActivityContent::ToolCall { tool }) => tool.clone(),
            _ => SessionToolCall::new(tool_call_id.clone()),
        };
        tool.apply_patch(patch);
        let content = SessionActivityContent::ToolCall { tool };
        let row = if let Some(row) = existing {
            self.activities
                .update_activity_content(&row.id, session_id, &content.summary(), &content)
                .await?
        } else {
            let row = self
                .append_structured_activity(
                    session_id,
                    stream.turn_id.as_deref(),
                    SessionActivityKind::ToolCall,
                    content,
                    Some(tool_call_id.clone()),
                )
                .await?;
            // A first occurrence anchors the tool between messages. Later field
            // updates must not split a text message currently streaming after it.
            stream.last = None;
            row
        };
        stream.tools.insert(tool_call_id, row);
        Ok(())
    }

    async fn append_structured_activity(
        &self,
        session_id: &str,
        turn: Option<&str>,
        kind: SessionActivityKind,
        content: SessionActivityContent,
        tool_call_id: Option<String>,
    ) -> Result<SessionActivity, SessionError> {
        Ok(self
            .activities
            .append_activity(NewSessionActivity {
                id: self.ids.new_id(),
                session_id: session_id.into(),
                turn_id: turn.map(Into::into),
                kind,
                text: content.summary(),
                content: Some(content),
                tool_call_id,
                created_at: self.clock.now_rfc3339(),
            })
            .await?)
    }
}
