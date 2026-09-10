CREATE TABLE session_recovery (
    session_id TEXT PRIMARY KEY NOT NULL REFERENCES managed_sessions(session_id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('never_started', 'unclosed', 'stopped', 'interrupted')),
    run_id TEXT,
    active_turn_id TEXT,
    interrupted_turn_id TEXT,
    last_error TEXT,
    updated_at TEXT NOT NULL,
    CHECK (status != 'unclosed' OR (run_id IS NOT NULL AND length(trim(run_id)) > 0)),
    CHECK (active_turn_id IS NULL OR (status = 'unclosed' AND length(trim(active_turn_id)) > 0)),
    CHECK (status != 'never_started' OR (run_id IS NULL AND active_turn_id IS NULL))
);

-- Older versions have no trustworthy run/turn checkpoint. A remote binding proves
-- a context existed, never that its process is still alive. Do not parse activity
-- text or manufacture a completed turn during migration.
INSERT INTO session_recovery(session_id, status, updated_at)
SELECT ms.session_id,
       CASE WHEN ms.remote_session_id IS NULL THEN 'never_started' ELSE 'interrupted' END,
       hs.updated_at
FROM managed_sessions ms JOIN host_sessions hs ON hs.id = ms.session_id;
