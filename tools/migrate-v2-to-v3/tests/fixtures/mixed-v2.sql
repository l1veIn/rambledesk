PRAGMA foreign_keys = OFF;

CREATE TABLE host_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    host_id TEXT NOT NULL,
    host_session_id TEXT NOT NULL,
    display_title TEXT,
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

CREATE TABLE request_actions (
    request_id TEXT NOT NULL,
    action_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    instruction TEXT NOT NULL,
    PRIMARY KEY (request_id, action_id),
    UNIQUE (request_id, position)
);

CREATE TABLE request_context_refs (
    request_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    label TEXT NOT NULL,
    uri TEXT NOT NULL,
    PRIMARY KEY (request_id, position)
);

CREATE TABLE attachments (
    id TEXT PRIMARY KEY NOT NULL,
    request_id TEXT NOT NULL,
    draft_path TEXT NOT NULL,
    published_path TEXT,
    file_name TEXT NOT NULL,
    byte_size INTEGER NOT NULL,
    media_type TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    position INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE (request_id, position)
);

CREATE TABLE request_attachments (
    id TEXT PRIMARY KEY NOT NULL,
    request_id TEXT NOT NULL,
    file_name TEXT NOT NULL,
    byte_size INTEGER NOT NULL,
    media_type TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    position INTEGER NOT NULL,
    contents BLOB NOT NULL,
    created_at TEXT NOT NULL,
    draft_path TEXT,
    published_path TEXT,
    UNIQUE (request_id, position)
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
    'session-1', 'generic', 'legacy-session', 'Migrated review session',
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
    'request-waiting', 'Waiting draft projection', 2,
    '2026-08-01T00:30:00Z', '{"schemaVersion":2,"doc":{"type":"doc","content":[{"type":"paragraph"}]}}'
);
INSERT INTO drafts VALUES (
    'request-in-progress', 'Unfinished structured feedback', 1,
    '2026-08-01T01:00:00Z', '{"schemaVersion":2,"doc":{"type":"doc"}}'
);
INSERT INTO drafts VALUES (
    'orphan-draft', 'No owning request', 3,
    '2026-08-01T07:00:00Z', '{"schemaVersion":2,"doc":{"type":"doc"}}'
);

INSERT INTO request_actions VALUES (
    'request-waiting', 'action-review', 0, 'Review the proposed implementation'
);
WITH RECURSIVE action_number(value) AS (
    VALUES(1)
    UNION ALL
    SELECT value + 1 FROM action_number WHERE value < 20
)
INSERT INTO request_actions
    (request_id, action_id, position, instruction)
SELECT
    'request-waiting',
    printf('action-extra-%02d', value),
    value,
    printf('Additional legacy action %02d', value)
FROM action_number;
INSERT INTO request_actions VALUES (
    'request-in-progress', '', 0, '   '
);
INSERT INTO request_context_refs VALUES (
    'request-waiting', 0, 'Relevant diff', 'https://example.invalid/review.diff'
);
INSERT INTO request_context_refs VALUES (
    'request-in-progress', 0, '', '   '
);
