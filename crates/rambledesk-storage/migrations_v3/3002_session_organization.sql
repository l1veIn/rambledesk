ALTER TABLE sessions_v3 RENAME COLUMN session_kind TO session_kind_v3001;

ALTER TABLE sessions_v3 ADD COLUMN session_kind TEXT NOT NULL DEFAULT 'managed'
    CHECK (session_kind IN ('managed', 'imported'));
ALTER TABLE sessions_v3 ADD COLUMN pinned_at TEXT;
ALTER TABLE sessions_v3 ADD COLUMN archived_at TEXT;

UPDATE sessions_v3
SET session_kind = CASE session_kind_v3001
    WHEN 'managed' THEN 'managed'
    WHEN 'connected' THEN 'imported'
END;

CREATE TRIGGER sessions_kind_projection_matches_v3
BEFORE INSERT ON sessions_v3
WHEN NOT (
    (NEW.session_kind = 'managed' AND NEW.session_kind_v3001 = 'managed')
    OR (NEW.session_kind = 'imported' AND NEW.session_kind_v3001 = 'connected')
)
BEGIN
    SELECT RAISE(ABORT, 'Session kind projection is inconsistent');
END;

CREATE TRIGGER sessions_kind_projection_update_matches_v3
BEFORE UPDATE OF session_kind, session_kind_v3001 ON sessions_v3
WHEN NOT (
    (NEW.session_kind = 'managed' AND NEW.session_kind_v3001 = 'managed')
    OR (NEW.session_kind = 'imported' AND NEW.session_kind_v3001 = 'connected')
)
BEGIN
    SELECT RAISE(ABORT, 'Session kind projection is inconsistent');
END;

CREATE TRIGGER sessions_projected_kind_is_immutable_v3
BEFORE UPDATE OF session_kind ON sessions_v3
BEGIN
    SELECT RAISE(ABORT, 'Session kind is immutable');
END;

CREATE TRIGGER sessions_organization_timestamps_are_valid_v3
BEFORE UPDATE OF pinned_at, archived_at ON sessions_v3
WHEN (NEW.pinned_at IS NOT NULL AND length(trim(NEW.pinned_at)) = 0)
  OR (NEW.archived_at IS NOT NULL AND length(trim(NEW.archived_at)) = 0)
BEGIN
    SELECT RAISE(ABORT, 'Session organization timestamps must be nonblank');
END;

DROP INDEX sessions_lifecycle_updated_v3;
CREATE INDEX sessions_active_organization_v3
    ON sessions_v3(archived_at, pinned_at DESC, updated_at DESC, session_id DESC);

UPDATE schema_generation_v3 SET revision = 2 WHERE singleton = 1;
