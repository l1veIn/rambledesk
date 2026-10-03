-- Preserve the released request attachment columns and rows while allowing
-- the supported audio/video containers. SQLite requires a table rebuild to
-- extend this CHECK; the migration runs in the opener's single transaction.
CREATE TABLE request_attachments_media (
    id TEXT PRIMARY KEY NOT NULL,
    request_id TEXT NOT NULL REFERENCES feedback_requests(id) ON DELETE CASCADE,
    file_name TEXT NOT NULL,
    byte_size INTEGER NOT NULL CHECK (byte_size > 0),
    media_type TEXT NOT NULL CHECK (
        media_type = 'text/markdown' OR media_type LIKE 'image/%'
        OR media_type IN ('audio/wav', 'audio/mpeg', 'audio/mp4', 'audio/ogg',
                         'audio/webm', 'video/mp4', 'video/webm', 'video/ogg')
    ),
    sha256 TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    contents BLOB NOT NULL,
    created_at TEXT NOT NULL,
    draft_path TEXT,
    published_path TEXT,
    UNIQUE (request_id, position)
);

INSERT INTO request_attachments_media
    (id, request_id, file_name, byte_size, media_type, sha256, position,
     contents, created_at, draft_path, published_path)
SELECT id, request_id, file_name, byte_size, media_type, sha256, position,
       contents, created_at, draft_path, published_path
FROM request_attachments;

DROP TABLE request_attachments;
ALTER TABLE request_attachments_media RENAME TO request_attachments;
CREATE INDEX request_attachments_request
    ON request_attachments(request_id, position);
