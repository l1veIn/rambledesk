ALTER TABLE submission_plans
    ADD COLUMN terminal_resolution TEXT NOT NULL DEFAULT 'feedback_submitted'
    CHECK (terminal_resolution IN ('feedback_submitted', 'cancelled'));

ALTER TABLE submission_plans ADD COLUMN cancel_reason TEXT;
