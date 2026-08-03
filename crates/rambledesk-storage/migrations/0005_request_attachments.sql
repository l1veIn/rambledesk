CREATE TABLE request_attachments (
    id TEXT PRIMARY KEY NOT NULL,
    request_id TEXT NOT NULL REFERENCES feedback_requests(id) ON DELETE CASCADE,
    file_name TEXT NOT NULL,
    byte_size INTEGER NOT NULL CHECK (byte_size > 0),
    media_type TEXT NOT NULL CHECK (
        media_type = 'text/markdown' OR media_type LIKE 'image/%'
    ),
    sha256 TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    contents BLOB NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE (request_id, position)
);

CREATE INDEX request_attachments_request
    ON request_attachments(request_id, position);
