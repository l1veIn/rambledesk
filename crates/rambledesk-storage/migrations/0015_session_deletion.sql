-- Deletion remains visible and retryable across a process exit between filesystem
-- cleanup and database commit. Only successful record deletion removes this intent.
CREATE TABLE session_deletions (
    session_id TEXT PRIMARY KEY NOT NULL REFERENCES managed_sessions(session_id) ON DELETE CASCADE,
    started_at TEXT NOT NULL
);
