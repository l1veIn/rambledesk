CREATE INDEX IF NOT EXISTS idx_session_activity_user_turn
    ON session_activity(session_id, sequence DESC)
    WHERE kind = 'user_message';
