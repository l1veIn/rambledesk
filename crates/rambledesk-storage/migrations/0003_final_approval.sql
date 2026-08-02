ALTER TABLE feedback_requests ADD COLUMN allow_finish INTEGER NOT NULL DEFAULT 0 CHECK (allow_finish IN (0, 1));
ALTER TABLE feedback_requests ADD COLUMN final_summary TEXT;
ALTER TABLE feedback_requests ADD COLUMN resolution TEXT CHECK (resolution IN ('feedback_submitted', 'approved', 'cancelled'));

CREATE TRIGGER feedback_requests_finish_proposal_is_valid
BEFORE INSERT ON feedback_requests
WHEN NEW.allow_finish = 1 AND (NEW.final_summary IS NULL OR length(trim(NEW.final_summary)) = 0)
BEGIN
    SELECT RAISE(ABORT, 'finish-enabled request requires final summary');
END;
