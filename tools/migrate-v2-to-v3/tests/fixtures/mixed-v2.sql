PRAGMA foreign_keys = OFF;

CREATE TABLE host_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    host_id TEXT NOT NULL,
    host_session_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE feedback_requests (
    id TEXT PRIMARY KEY NOT NULL,
    host_session_record_id TEXT NOT NULL,
    title TEXT NOT NULL,
    what_happened TEXT NOT NULL,
    status TEXT NOT NULL,
    revision INTEGER NOT NULL,
    input_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    cancelled_at TEXT,
    cancel_reason TEXT,
    allow_finish INTEGER NOT NULL DEFAULT 0,
    final_summary TEXT,
    resolution TEXT
);

CREATE TABLE drafts (
    request_id TEXT PRIMARY KEY NOT NULL,
    body_markdown TEXT NOT NULL,
    revision INTEGER NOT NULL,
    updated_at TEXT NOT NULL,
    document_json TEXT
);

CREATE TABLE feedback_results (
    request_id TEXT PRIMARY KEY NOT NULL,
    package_uri TEXT NOT NULL,
    directory_path TEXT NOT NULL,
    markdown_path TEXT NOT NULL,
    manifest_path TEXT NOT NULL,
    manifest_sha256 TEXT NOT NULL,
    published_at TEXT NOT NULL
);

INSERT INTO host_sessions VALUES (
    'session-1', 'generic', 'legacy-session',
    '2026-08-01T00:00:00Z', '2026-08-02T00:00:00Z'
);

INSERT INTO feedback_requests
    (id, host_session_record_id, title, what_happened, status, revision,
     input_hash, created_at, updated_at, completed_at, cancelled_at,
     cancel_reason, allow_finish, final_summary, resolution)
VALUES
    ('request-waiting', 'session-1', 'Waiting', 'Waiting request', 'waiting', 0,
     'hash', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z', NULL, NULL,
     NULL, 0, NULL, NULL),
    ('request-in-progress', 'session-1', 'Drafting', 'Drafting request', 'in_progress', 1,
     'hash', '2026-08-01T00:00:00Z', '2026-08-01T01:00:00Z', NULL, NULL,
     NULL, 0, NULL, NULL),
    ('request-completed-readable', 'session-1', 'Completed', 'Readable package', 'completed', 1,
     'hash', '2026-08-01T00:00:00Z', '2026-08-01T02:00:00Z', '2026-08-01T02:00:00Z', NULL,
     NULL, 0, NULL, 'feedback_submitted'),
    ('request-completed-unreadable', 'session-1', 'Broken', 'Unreadable package', 'completed', 1,
     'hash', '2026-08-01T00:00:00Z', '2026-08-01T03:00:00Z', '2026-08-01T03:00:00Z', NULL,
     NULL, 0, NULL, 'feedback_submitted'),
    ('request-cancelled', 'session-1', 'Cancelled', 'Cancelled request', 'cancelled', 1,
     'hash', '2026-08-01T00:00:00Z', '2026-08-01T04:00:00Z', NULL, '2026-08-01T04:00:00Z',
     'obsolete', 0, NULL, 'cancelled'),
    ('request-approved', 'session-1', 'Approved', 'Approval shortcut', 'completed', 1,
     'hash', '2026-08-01T00:00:00Z', '2026-08-01T05:00:00Z', '2026-08-01T05:00:00Z', NULL,
     NULL, 1, 'done', 'approved'),
    ('request-allow-finish', 'session-1', 'Allow finish', 'Ambiguous approval', 'waiting', 0,
     'hash', '2026-08-01T00:00:00Z', '2026-08-01T06:00:00Z', NULL, NULL,
     NULL, 1, 'proposed finish', NULL);

INSERT INTO drafts VALUES (
    'request-in-progress', 'Unfinished structured feedback', 1,
    '2026-08-01T01:00:00Z', '{"schemaVersion":2,"doc":{"type":"doc"}}'
);
INSERT INTO drafts VALUES (
    'orphan-draft', 'No owning request', 3,
    '2026-08-01T07:00:00Z', '{"schemaVersion":2,"doc":{"type":"doc"}}'
);
