CREATE TABLE feedback_deliveries (
    request_id TEXT PRIMARY KEY NOT NULL REFERENCES feedback_requests(id) ON DELETE CASCADE,
    session_id TEXT NOT NULL REFERENCES managed_sessions(session_id) ON DELETE CASCADE,
    resolution TEXT NOT NULL CHECK (resolution IN ('feedback_submitted', 'approved', 'cancelled')),
    state TEXT NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending', 'sending', 'delivered', 'uncertain', 'discarded')),
    attempt_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_error TEXT,
    CHECK (state != 'sending' OR (attempt_id IS NOT NULL AND length(trim(attempt_id)) > 0))
);

CREATE INDEX feedback_deliveries_pending ON feedback_deliveries(state, created_at, request_id);
CREATE INDEX feedback_deliveries_session ON feedback_deliveries(session_id, created_at, request_id);

-- Reconcile requests completed before this migration. Existing external requests
-- have no trusted managed marker, so they never acquire automatic continuation.
INSERT INTO feedback_deliveries (request_id, session_id, resolution, created_at, updated_at)
SELECT id, managed_session_id, resolution, updated_at, updated_at FROM feedback_requests
WHERE managed_session_id IS NOT NULL AND status IN ('completed', 'cancelled')
  AND resolution IN ('feedback_submitted', 'approved', 'cancelled');

CREATE TRIGGER feedback_deliveries_require_terminal_managed_request
BEFORE INSERT ON feedback_deliveries
WHEN NOT EXISTS (
    SELECT 1 FROM feedback_requests r WHERE r.id = NEW.request_id
      AND r.managed_session_id = NEW.session_id AND r.resolution = NEW.resolution
      AND r.status IN ('completed', 'cancelled')
)
BEGIN
    SELECT RAISE(ABORT, 'delivery requires the owned terminal managed request');
END;

CREATE TRIGGER feedback_deliveries_ownership_is_immutable
BEFORE UPDATE OF request_id, session_id, resolution ON feedback_deliveries
WHEN NEW.request_id IS NOT OLD.request_id OR NEW.session_id IS NOT OLD.session_id
  OR NEW.resolution IS NOT OLD.resolution
BEGIN
    SELECT RAISE(ABORT, 'delivery ownership is immutable');
END;
