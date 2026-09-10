-- Only the trusted scoped feedback application writes this delivery marker.
-- NULL preserves all legacy and external adapter semantics.
ALTER TABLE feedback_requests ADD COLUMN managed_session_id TEXT
    REFERENCES managed_sessions(session_id) ON DELETE CASCADE
    CHECK (managed_session_id IS NULL OR managed_session_id = host_session_record_id);

CREATE INDEX feedback_requests_managed_session ON feedback_requests(managed_session_id);

-- A request cannot opt into, or opt out of, its original delivery ownership.
CREATE TRIGGER feedback_requests_managed_scope_is_immutable
BEFORE UPDATE OF managed_session_id, host_session_record_id ON feedback_requests
WHEN NEW.managed_session_id IS NOT OLD.managed_session_id
  OR NEW.host_session_record_id IS NOT OLD.host_session_record_id
BEGIN
    SELECT RAISE(ABORT, 'feedback request session ownership is immutable');
END;
