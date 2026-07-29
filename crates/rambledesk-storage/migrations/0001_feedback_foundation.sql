PRAGMA foreign_keys = ON;

CREATE TABLE projects (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL,
    root_path_canonical TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE agent_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id),
    agent TEXT NOT NULL,
    external_session_id TEXT NOT NULL,
    ended_at TEXT,
    created_at TEXT NOT NULL,
    UNIQUE(project_id, agent, external_session_id)
);

CREATE TABLE feedback_requests (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES agent_sessions(id),
    what_happened TEXT NOT NULL,
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
    media_type TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    created_at TEXT NOT NULL,
    UNIQUE (request_id, position)
);

CREATE TABLE invocation_attempts (
    id TEXT PRIMARY KEY NOT NULL,
    request_id TEXT REFERENCES feedback_requests(id),
    transport_request_id TEXT,
    execution_mode TEXT NOT NULL CHECK (execution_mode IN ('poll', 'task')),
    status TEXT NOT NULL CHECK (status IN ('open', 'responded', 'disconnected', 'cancelled', 'failed')),
    opened_at TEXT NOT NULL,
    closed_at TEXT,
    error_code TEXT
);

CREATE TABLE completion_notifications (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES agent_sessions(id),
    summary TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE (session_id, input_hash)
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

CREATE TABLE outbox_events (
    id TEXT PRIMARY KEY NOT NULL,
    event_type TEXT NOT NULL,
    aggregate_id TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    delivered_at TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    last_error_code TEXT
);

CREATE INDEX feedback_requests_status_updated
    ON feedback_requests(status, updated_at DESC, id DESC);
CREATE INDEX agent_sessions_project
    ON agent_sessions(project_id, created_at DESC);
CREATE INDEX outbox_events_pending
    ON outbox_events(delivered_at, created_at)
    WHERE delivered_at IS NULL;
