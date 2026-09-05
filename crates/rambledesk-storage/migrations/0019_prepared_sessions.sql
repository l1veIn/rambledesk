ALTER TABLE managed_sessions ADD COLUMN lifecycle TEXT NOT NULL DEFAULT 'active'
    CHECK (lifecycle IN ('prepared', 'active'));

CREATE INDEX managed_sessions_lifecycle ON managed_sessions(lifecycle, session_id);
