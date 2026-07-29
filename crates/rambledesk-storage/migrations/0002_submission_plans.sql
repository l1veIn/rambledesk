CREATE TABLE submission_plans (
    request_id TEXT PRIMARY KEY NOT NULL REFERENCES feedback_requests(id),
    publication_id TEXT NOT NULL UNIQUE,
    source_revision INTEGER NOT NULL CHECK (source_revision > 0),
    body_sha256 TEXT NOT NULL,
    submitted_at TEXT NOT NULL,
    package_uri TEXT NOT NULL,
    directory_path TEXT NOT NULL UNIQUE,
    temp_directory_path TEXT NOT NULL UNIQUE,
    markdown_path TEXT NOT NULL,
    manifest_path TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'preparing' CHECK (state IN ('preparing', 'published')),
    manifest_sha256 TEXT,
    published_at TEXT,
    last_error_code TEXT,
    last_error_at TEXT,
    CHECK (
        (state = 'preparing' AND manifest_sha256 IS NULL AND published_at IS NULL)
        OR
        (state = 'published' AND manifest_sha256 IS NOT NULL AND published_at IS NOT NULL)
    )
);

CREATE TRIGGER drafts_locked_after_submission_plan_update
BEFORE UPDATE ON drafts
WHEN EXISTS (
    SELECT 1 FROM submission_plans WHERE request_id = OLD.request_id
)
BEGIN
    SELECT RAISE(ABORT, 'draft is locked by a submission plan');
END;

CREATE TRIGGER drafts_locked_after_submission_plan_delete
BEFORE DELETE ON drafts
WHEN EXISTS (
    SELECT 1 FROM submission_plans WHERE request_id = OLD.request_id
)
BEGIN
    SELECT RAISE(ABORT, 'draft is locked by a submission plan');
END;
