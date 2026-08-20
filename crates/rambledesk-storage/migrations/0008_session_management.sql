ALTER TABLE host_sessions ADD COLUMN display_title TEXT;
ALTER TABLE host_sessions ADD COLUMN pinned_at TEXT;
ALTER TABLE host_sessions ADD COLUMN archived_at TEXT;

CREATE TABLE host_preferences (
    host_id TEXT PRIMARY KEY NOT NULL,
    pinned_at TEXT,
    updated_at TEXT NOT NULL
);

CREATE INDEX host_sessions_archive_pin_updated
    ON host_sessions(archived_at, pinned_at DESC, updated_at DESC);
CREATE INDEX host_preferences_pin
    ON host_preferences(pinned_at DESC, host_id);
