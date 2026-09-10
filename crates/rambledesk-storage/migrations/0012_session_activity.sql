CREATE TABLE session_activity (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES managed_sessions(session_id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    turn_id TEXT,
    kind TEXT NOT NULL CHECK (kind IN (
        'user_message', 'agent_message', 'agent_thought', 'tool_call', 'status', 'error'
    )),
    text TEXT NOT NULL,
    tool_call_id TEXT,
    created_at TEXT NOT NULL,
    UNIQUE (session_id, sequence)
);
