PRAGMA foreign_keys = ON;

CREATE TABLE host_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    host_id TEXT NOT NULL,
    host_session_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(host_id, host_session_id)
);

CREATE TABLE feedback_requests (
    id TEXT PRIMARY KEY NOT NULL,
    host_session_record_id TEXT NOT NULL REFERENCES host_sessions(id),
    title TEXT NOT NULL,
    what_happened TEXT NOT NULL,
    source_hint TEXT,
    status TEXT NOT NULL CHECK (status IN ('waiting', 'in_progress', 'completed', 'cancelled')),
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
    input_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    cancelled_at TEXT,
    cancel_reason TEXT,
    CHECK ((status = 'completed' AND completed_at IS NOT NULL) OR status != 'completed'),
    CHECK ((status = 'cancelled' AND cancelled_at IS NOT NULL) OR status != 'cancelled')
);

CREATE TRIGGER feedback_requests_completed_is_terminal
BEFORE UPDATE OF status ON feedback_requests
WHEN OLD.status = 'completed' AND NEW.status != 'completed'
BEGIN
    SELECT RAISE(ABORT, 'completed feedback request is terminal');
END;

CREATE TRIGGER feedback_requests_cancelled_is_terminal
BEFORE UPDATE OF status ON feedback_requests
WHEN OLD.status = 'cancelled' AND NEW.status != 'cancelled'
BEGIN
    SELECT RAISE(ABORT, 'cancelled feedback request is terminal');
END;

CREATE TABLE request_actions (
    request_id TEXT NOT NULL REFERENCES feedback_requests(id) ON DELETE CASCADE,
    action_id TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    instruction TEXT NOT NULL,
    PRIMARY KEY (request_id, action_id),
    UNIQUE (request_id, position)
);

CREATE TABLE request_context_refs (
    request_id TEXT NOT NULL REFERENCES feedback_requests(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    label TEXT NOT NULL,
    uri TEXT NOT NULL,
    PRIMARY KEY (request_id, position)
);

CREATE TABLE drafts (
    request_id TEXT PRIMARY KEY NOT NULL REFERENCES feedback_requests(id) ON DELETE CASCADE,
    body_markdown TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 0),
    updated_at TEXT NOT NULL
);

CREATE TABLE attachments (
    id TEXT PRIMARY KEY NOT NULL,
    request_id TEXT NOT NULL REFERENCES feedback_requests(id) ON DELETE CASCADE,
    draft_path TEXT NOT NULL,
    published_path TEXT,
    file_name TEXT NOT NULL,
    byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
    media_type TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    created_at TEXT NOT NULL,
    UNIQUE (request_id, position)
);

CREATE TABLE feedback_results (
    request_id TEXT PRIMARY KEY NOT NULL REFERENCES feedback_requests(id),
    package_uri TEXT NOT NULL,
    directory_path TEXT NOT NULL,
    markdown_path TEXT NOT NULL,
    manifest_path TEXT NOT NULL,
    manifest_sha256 TEXT NOT NULL,
    published_at TEXT NOT NULL
);

CREATE INDEX feedback_requests_status_updated
    ON feedback_requests(status, updated_at DESC, id DESC);
CREATE INDEX feedback_requests_host_session_updated
    ON feedback_requests(host_session_record_id, updated_at DESC);
CREATE INDEX host_sessions_host
    ON host_sessions(host_id, created_at DESC);
