-- Legacy activity remains readable through its existing text summary.
ALTER TABLE session_activity ADD COLUMN content_json TEXT
    CHECK (content_json IS NULL OR json_valid(content_json));
