PRAGMA foreign_keys = ON;

CREATE TABLE schema_generation_v3 (
    singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
    generation INTEGER NOT NULL CHECK (generation = 3),
    revision INTEGER NOT NULL CHECK (revision >= 1)
);

INSERT INTO schema_generation_v3 (singleton, generation, revision)
VALUES (1, 3, 1);

CREATE TABLE sessions_v3 (
    session_id TEXT PRIMARY KEY NOT NULL,
    session_kind TEXT NOT NULL CHECK (session_kind IN ('managed', 'connected')),
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    lifecycle TEXT NOT NULL CHECK (lifecycle IN ('ready', 'stopped', 'failed')),
    launch_configuration_json TEXT,
    created_at TEXT NOT NULL CHECK (length(created_at) > 0),
    updated_at TEXT NOT NULL CHECK (length(updated_at) > 0),
    CHECK (
        (session_kind = 'managed'
            AND launch_configuration_json IS NOT NULL
            AND json_valid(launch_configuration_json))
        OR
        (session_kind = 'connected' AND launch_configuration_json IS NULL)
    )
);

CREATE INDEX sessions_lifecycle_updated_v3
    ON sessions_v3(lifecycle, updated_at DESC, session_id DESC);

CREATE TRIGGER sessions_identity_is_immutable_v3
BEFORE UPDATE OF session_kind, launch_configuration_json ON sessions_v3
BEGIN
    SELECT RAISE(ABORT, 'Session kind and Launch Configuration are immutable');
END;

CREATE TABLE acp_session_links_v3 (
    link_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES sessions_v3(session_id) ON DELETE CASCADE,
    agent_profile_id TEXT NOT NULL CHECK (length(trim(agent_profile_id)) > 0),
    launch_profile_id TEXT NOT NULL CHECK (length(trim(launch_profile_id)) > 0),
    acp_session_id TEXT NOT NULL CHECK (length(acp_session_id) > 0),
    capabilities_json TEXT NOT NULL,
    session_toolset_digest TEXT NOT NULL CHECK (
        length(session_toolset_digest) = 71
        AND substr(session_toolset_digest, 1, 7) = 'sha256:'
        AND substr(session_toolset_digest, 8) NOT GLOB '*[^0-9a-f]*'
    ),
    is_current INTEGER NOT NULL CHECK (is_current IN (0, 1)),
    created_at TEXT NOT NULL CHECK (length(created_at) > 0),
    last_used_at TEXT NOT NULL CHECK (length(last_used_at) > 0),
    UNIQUE (agent_profile_id, launch_profile_id, acp_session_id),
    UNIQUE (session_id, link_id)
);

CREATE UNIQUE INDEX acp_session_links_one_current_v3
    ON acp_session_links_v3(session_id)
    WHERE is_current = 1;

CREATE INDEX acp_session_links_session_used_v3
    ON acp_session_links_v3(session_id, last_used_at DESC, link_id DESC);

CREATE TRIGGER acp_session_links_require_managed_session_v3
BEFORE INSERT ON acp_session_links_v3
WHEN NOT EXISTS (
    SELECT 1
    FROM sessions_v3
    WHERE session_id = NEW.session_id AND session_kind = 'managed'
)
BEGIN
    SELECT RAISE(ABORT, 'ACP Session Link requires a Managed Session');
END;

CREATE TABLE artifact_objects_v3 (
    storage_key TEXT PRIMARY KEY NOT NULL CHECK (length(storage_key) > 0),
    sha256 TEXT NOT NULL UNIQUE CHECK (
        length(sha256) = 71
        AND substr(sha256, 1, 7) = 'sha256:'
        AND substr(sha256, 8) NOT GLOB '*[^0-9a-f]*'
    ),
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    created_at TEXT NOT NULL CHECK (length(created_at) > 0),
    UNIQUE (storage_key, sha256, size_bytes)
);

CREATE TRIGGER artifact_objects_are_immutable_v3
BEFORE UPDATE ON artifact_objects_v3
BEGIN
    SELECT RAISE(ABORT, 'Artifact Object is immutable');
END;

CREATE TABLE feedback_requests_v3 (
    request_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    source_link_id TEXT,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    instructions TEXT NOT NULL CHECK (length(trim(instructions)) > 0),
    input_digest TEXT NOT NULL CHECK (
        length(input_digest) = 71
        AND substr(input_digest, 1, 7) = 'sha256:'
        AND substr(input_digest, 8) NOT GLOB '*[^0-9a-f]*'
    ),
    resolution TEXT CHECK (resolution IN ('submitted', 'cancelled')),
    response_package_id TEXT,
    cancel_reason TEXT,
    created_at TEXT NOT NULL CHECK (length(created_at) > 0),
    resolved_at TEXT,
    updated_at TEXT NOT NULL CHECK (length(updated_at) > 0),
    UNIQUE (session_id, request_id),
    FOREIGN KEY (session_id)
        REFERENCES sessions_v3(session_id) ON DELETE RESTRICT,
    FOREIGN KEY (session_id, source_link_id)
        REFERENCES acp_session_links_v3(session_id, link_id) ON DELETE RESTRICT,
    FOREIGN KEY (request_id, response_package_id)
        REFERENCES packages_v3(request_id, package_id)
        ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED,
    CHECK (
        (resolution IS NULL
            AND response_package_id IS NULL
            AND cancel_reason IS NULL
            AND resolved_at IS NULL)
        OR
        (resolution = 'submitted'
            AND response_package_id IS NOT NULL
            AND cancel_reason IS NULL
            AND resolved_at IS NOT NULL
            AND length(resolved_at) > 0)
        OR
        (resolution = 'cancelled'
            AND response_package_id IS NULL
            AND cancel_reason IS NOT NULL
            AND length(trim(cancel_reason)) > 0
            AND resolved_at IS NOT NULL
            AND length(resolved_at) > 0)
    )
);

CREATE INDEX feedback_requests_waiting_v3
    ON feedback_requests_v3(session_id, updated_at DESC, request_id DESC)
    WHERE resolution IS NULL;

CREATE INDEX feedback_requests_resolved_v3
    ON feedback_requests_v3(session_id, resolved_at DESC, request_id DESC)
    WHERE resolution IS NOT NULL;

CREATE TRIGGER feedback_requests_resolution_is_terminal_v3
BEFORE UPDATE ON feedback_requests_v3
WHEN OLD.resolution IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'Feedback Request resolution is terminal');
END;

CREATE TRIGGER feedback_requests_input_is_immutable_v3
BEFORE UPDATE OF session_id, source_link_id, title, instructions, input_digest
ON feedback_requests_v3
BEGIN
    SELECT RAISE(ABORT, 'Feedback Request input is immutable');
END;

CREATE TABLE feedback_request_actions_v3 (
    request_id TEXT NOT NULL
        REFERENCES feedback_requests_v3(request_id) ON DELETE CASCADE
        DEFERRABLE INITIALLY DEFERRED,
    action_id TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position BETWEEN 0 AND 19),
    instruction TEXT NOT NULL CHECK (length(trim(instruction)) > 0),
    PRIMARY KEY (request_id, action_id),
    UNIQUE (request_id, position)
);

CREATE TRIGGER feedback_request_actions_no_update_v3
BEFORE UPDATE ON feedback_request_actions_v3
BEGIN SELECT RAISE(ABORT, 'Feedback Request Action is immutable'); END;

CREATE TRIGGER feedback_request_actions_no_delete_v3
BEFORE DELETE ON feedback_request_actions_v3
BEGIN SELECT RAISE(ABORT, 'Feedback Request Action is immutable'); END;

CREATE TRIGGER feedback_request_actions_no_late_insert_v3
BEFORE INSERT ON feedback_request_actions_v3
WHEN EXISTS (SELECT 1 FROM feedback_requests_v3 WHERE request_id = NEW.request_id)
BEGIN SELECT RAISE(ABORT, 'Feedback Request Action cannot be appended'); END;

CREATE TABLE feedback_request_context_refs_v3 (
    request_id TEXT NOT NULL
        REFERENCES feedback_requests_v3(request_id) ON DELETE CASCADE
        DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL CHECK (position >= 0),
    label TEXT NOT NULL CHECK (length(trim(label)) > 0),
    uri TEXT NOT NULL CHECK (length(trim(uri)) > 0),
    PRIMARY KEY (request_id, position)
);

CREATE TRIGGER feedback_request_context_refs_no_update_v3
BEFORE UPDATE ON feedback_request_context_refs_v3
BEGIN SELECT RAISE(ABORT, 'Feedback Request Context Reference is immutable'); END;

CREATE TRIGGER feedback_request_context_refs_no_delete_v3
BEFORE DELETE ON feedback_request_context_refs_v3
BEGIN SELECT RAISE(ABORT, 'Feedback Request Context Reference is immutable'); END;

CREATE TRIGGER feedback_request_context_refs_no_late_insert_v3
BEFORE INSERT ON feedback_request_context_refs_v3
WHEN EXISTS (SELECT 1 FROM feedback_requests_v3 WHERE request_id = NEW.request_id)
BEGIN SELECT RAISE(ABORT, 'Feedback Request Context Reference cannot be appended'); END;

CREATE TABLE feedback_request_artifacts_v3 (
    request_id TEXT NOT NULL
        REFERENCES feedback_requests_v3(request_id) ON DELETE CASCADE
        DEFERRABLE INITIALLY DEFERRED,
    artifact_id TEXT NOT NULL CHECK (length(artifact_id) > 0),
    position INTEGER NOT NULL CHECK (position >= 0),
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) > 0),
    media_type TEXT NOT NULL CHECK (length(trim(media_type)) > 0),
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    sha256 TEXT NOT NULL CHECK (
        length(sha256) = 71
        AND substr(sha256, 1, 7) = 'sha256:'
        AND substr(sha256, 8) NOT GLOB '*[^0-9a-f]*'
    ),
    storage_key TEXT NOT NULL,
    PRIMARY KEY (request_id, artifact_id),
    UNIQUE (request_id, position),
    FOREIGN KEY (storage_key, sha256, size_bytes)
        REFERENCES artifact_objects_v3(storage_key, sha256, size_bytes)
        ON DELETE RESTRICT
);

CREATE TRIGGER feedback_request_artifacts_no_update_v3
BEFORE UPDATE ON feedback_request_artifacts_v3
BEGIN SELECT RAISE(ABORT, 'Feedback Request Artifact is immutable'); END;

CREATE TRIGGER feedback_request_artifacts_no_delete_v3
BEFORE DELETE ON feedback_request_artifacts_v3
BEGIN SELECT RAISE(ABORT, 'Feedback Request Artifact is immutable'); END;

CREATE TRIGGER feedback_request_artifacts_no_late_insert_v3
BEFORE INSERT ON feedback_request_artifacts_v3
WHEN EXISTS (SELECT 1 FROM feedback_requests_v3 WHERE request_id = NEW.request_id)
BEGIN SELECT RAISE(ABORT, 'Feedback Request Artifact cannot be appended'); END;

CREATE TABLE ramble_submissions_v3 (
    submission_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    intent TEXT NOT NULL CHECK (intent IN ('launch', 'steering', 'feedback')),
    request_id TEXT,
    document_json TEXT NOT NULL,
    body_markdown TEXT NOT NULL,
    submission_digest TEXT NOT NULL CHECK (
        length(submission_digest) = 71
        AND substr(submission_digest, 1, 7) = 'sha256:'
        AND substr(submission_digest, 8) NOT GLOB '*[^0-9a-f]*'
    ),
    created_at TEXT NOT NULL CHECK (length(created_at) > 0),
    UNIQUE (session_id, submission_id),
    FOREIGN KEY (session_id)
        REFERENCES sessions_v3(session_id) ON DELETE RESTRICT,
    FOREIGN KEY (session_id, request_id)
        REFERENCES feedback_requests_v3(session_id, request_id) ON DELETE RESTRICT,
    CHECK (
        (intent IN ('launch', 'steering') AND request_id IS NULL)
        OR
        (intent = 'feedback' AND request_id IS NOT NULL)
    )
);

CREATE UNIQUE INDEX ramble_submissions_one_launch_v3
    ON ramble_submissions_v3(session_id)
    WHERE intent = 'launch';

CREATE UNIQUE INDEX ramble_submissions_one_feedback_v3
    ON ramble_submissions_v3(request_id)
    WHERE intent = 'feedback';

CREATE INDEX ramble_submissions_session_created_v3
    ON ramble_submissions_v3(session_id, created_at, submission_id);

CREATE TRIGGER ramble_submissions_are_immutable_v3
BEFORE UPDATE ON ramble_submissions_v3
BEGIN
    SELECT RAISE(ABORT, 'Ramble Submission is immutable');
END;

CREATE TABLE submission_artifacts_v3 (
    submission_id TEXT NOT NULL
        REFERENCES ramble_submissions_v3(submission_id) ON DELETE RESTRICT
        DEFERRABLE INITIALLY DEFERRED,
    artifact_id TEXT NOT NULL CHECK (length(artifact_id) > 0),
    position INTEGER NOT NULL CHECK (position >= 0),
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) > 0),
    media_type TEXT NOT NULL CHECK (length(trim(media_type)) > 0),
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    sha256 TEXT NOT NULL CHECK (
        length(sha256) = 71
        AND substr(sha256, 1, 7) = 'sha256:'
        AND substr(sha256, 8) NOT GLOB '*[^0-9a-f]*'
    ),
    storage_key TEXT NOT NULL,
    PRIMARY KEY (submission_id, artifact_id),
    UNIQUE (submission_id, position),
    FOREIGN KEY (storage_key, sha256, size_bytes)
        REFERENCES artifact_objects_v3(storage_key, sha256, size_bytes)
        ON DELETE RESTRICT
);

CREATE TRIGGER submission_artifacts_no_update_v3
BEFORE UPDATE ON submission_artifacts_v3
BEGIN SELECT RAISE(ABORT, 'Submission Artifact is immutable'); END;

CREATE TRIGGER submission_artifacts_no_delete_v3
BEFORE DELETE ON submission_artifacts_v3
BEGIN SELECT RAISE(ABORT, 'Submission Artifact is immutable'); END;

CREATE TRIGGER submission_artifacts_no_late_insert_v3
BEFORE INSERT ON submission_artifacts_v3
WHEN EXISTS (SELECT 1 FROM ramble_submissions_v3 WHERE submission_id = NEW.submission_id)
BEGIN SELECT RAISE(ABORT, 'Submission Artifact cannot be appended'); END;

CREATE TABLE ramble_drafts_v3 (
    draft_id TEXT PRIMARY KEY NOT NULL,
    intent TEXT NOT NULL CHECK (intent IN ('launch', 'steering', 'feedback')),
    session_id TEXT,
    request_id TEXT,
    launch_configuration_json TEXT,
    document_json TEXT NOT NULL,
    body_markdown TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 0),
    created_at TEXT NOT NULL CHECK (length(created_at) > 0),
    updated_at TEXT NOT NULL CHECK (length(updated_at) > 0),
    FOREIGN KEY (session_id)
        REFERENCES sessions_v3(session_id) ON DELETE CASCADE,
    FOREIGN KEY (session_id, request_id)
        REFERENCES feedback_requests_v3(session_id, request_id) ON DELETE CASCADE,
    CHECK (
        (intent = 'launch'
            AND session_id IS NULL
            AND request_id IS NULL
            AND launch_configuration_json IS NOT NULL
            AND json_valid(launch_configuration_json))
        OR
        (intent = 'steering'
            AND session_id IS NOT NULL
            AND request_id IS NULL
            AND launch_configuration_json IS NULL)
        OR
        (intent = 'feedback'
            AND session_id IS NOT NULL
            AND request_id IS NOT NULL
            AND launch_configuration_json IS NULL)
    )
);

CREATE UNIQUE INDEX ramble_drafts_one_steering_v3
    ON ramble_drafts_v3(session_id)
    WHERE intent = 'steering';

CREATE UNIQUE INDEX ramble_drafts_one_feedback_v3
    ON ramble_drafts_v3(request_id)
    WHERE intent = 'feedback';

CREATE INDEX ramble_drafts_updated_v3
    ON ramble_drafts_v3(updated_at DESC, draft_id DESC);

CREATE TRIGGER ramble_drafts_identity_is_immutable_v3
BEFORE UPDATE OF intent, session_id, request_id ON ramble_drafts_v3
BEGIN SELECT RAISE(ABORT, 'Ramble Draft identity is immutable'); END;

CREATE TABLE draft_artifacts_v3 (
    draft_id TEXT NOT NULL
        REFERENCES ramble_drafts_v3(draft_id) ON DELETE CASCADE,
    artifact_id TEXT NOT NULL CHECK (length(artifact_id) > 0),
    position INTEGER NOT NULL CHECK (position >= 0),
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) > 0),
    media_type TEXT NOT NULL CHECK (length(trim(media_type)) > 0),
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    sha256 TEXT NOT NULL CHECK (
        length(sha256) = 71
        AND substr(sha256, 1, 7) = 'sha256:'
        AND substr(sha256, 8) NOT GLOB '*[^0-9a-f]*'
    ),
    storage_key TEXT NOT NULL,
    PRIMARY KEY (draft_id, artifact_id),
    UNIQUE (draft_id, position),
    FOREIGN KEY (storage_key, sha256, size_bytes)
        REFERENCES artifact_objects_v3(storage_key, sha256, size_bytes)
        ON DELETE RESTRICT
);

CREATE TABLE packages_v3 (
    package_id TEXT PRIMARY KEY NOT NULL,
    submission_id TEXT NOT NULL UNIQUE
        REFERENCES ramble_submissions_v3(submission_id) ON DELETE RESTRICT,
    package_purpose TEXT NOT NULL CHECK (package_purpose IN ('launch', 'response')),
    request_id TEXT,
    schema_version INTEGER NOT NULL CHECK (schema_version = 3),
    manifest_json TEXT NOT NULL CHECK (json_valid(manifest_json)),
    content_digest TEXT NOT NULL CHECK (
        length(content_digest) = 71
        AND substr(content_digest, 1, 7) = 'sha256:'
        AND substr(content_digest, 8) NOT GLOB '*[^0-9a-f]*'
    ),
    manifest_digest TEXT NOT NULL CHECK (
        length(manifest_digest) = 71
        AND substr(manifest_digest, 1, 7) = 'sha256:'
        AND substr(manifest_digest, 8) NOT GLOB '*[^0-9a-f]*'
    ),
    published_at TEXT NOT NULL CHECK (length(published_at) > 0),
    UNIQUE (request_id, package_id),
    FOREIGN KEY (request_id)
        REFERENCES feedback_requests_v3(request_id) ON DELETE RESTRICT,
    CHECK (
        (package_purpose = 'launch' AND request_id IS NULL)
        OR
        (package_purpose = 'response' AND request_id IS NOT NULL)
    )
);

CREATE UNIQUE INDEX packages_one_response_v3
    ON packages_v3(request_id)
    WHERE package_purpose = 'response';

CREATE INDEX packages_published_v3
    ON packages_v3(published_at DESC, package_id DESC);

CREATE TRIGGER packages_no_update_v3
BEFORE UPDATE ON packages_v3
BEGIN SELECT RAISE(ABORT, 'Package is immutable'); END;

CREATE TRIGGER packages_no_delete_v3
BEFORE DELETE ON packages_v3
BEGIN SELECT RAISE(ABORT, 'Package is immutable'); END;

CREATE TRIGGER packages_match_launch_submission_v3
BEFORE INSERT ON packages_v3
WHEN NEW.package_purpose = 'launch' AND NOT EXISTS (
    SELECT 1
    FROM ramble_submissions_v3
    WHERE submission_id = NEW.submission_id
      AND intent = 'launch'
      AND request_id IS NULL
)
BEGIN
    SELECT RAISE(ABORT, 'Launch Package must match a Launch Submission');
END;

CREATE TRIGGER packages_match_feedback_submission_v3
BEFORE INSERT ON packages_v3
WHEN NEW.package_purpose = 'response' AND NOT EXISTS (
    SELECT 1
    FROM ramble_submissions_v3
    WHERE submission_id = NEW.submission_id
      AND intent = 'feedback'
      AND request_id = NEW.request_id
)
BEGIN
    SELECT RAISE(ABORT, 'Response Package must match a Feedback Submission');
END;

CREATE TABLE package_artifacts_v3 (
    package_id TEXT NOT NULL
        REFERENCES packages_v3(package_id) ON DELETE RESTRICT
        DEFERRABLE INITIALLY DEFERRED,
    artifact_id TEXT NOT NULL CHECK (length(artifact_id) > 0),
    position INTEGER NOT NULL CHECK (position >= 0),
    role TEXT NOT NULL CHECK (length(trim(role)) > 0),
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) > 0),
    media_type TEXT NOT NULL CHECK (length(trim(media_type)) > 0),
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    sha256 TEXT NOT NULL CHECK (
        length(sha256) = 71
        AND substr(sha256, 1, 7) = 'sha256:'
        AND substr(sha256, 8) NOT GLOB '*[^0-9a-f]*'
    ),
    storage_key TEXT NOT NULL,
    PRIMARY KEY (package_id, artifact_id),
    UNIQUE (package_id, position),
    FOREIGN KEY (storage_key, sha256, size_bytes)
        REFERENCES artifact_objects_v3(storage_key, sha256, size_bytes)
        ON DELETE RESTRICT
);

CREATE TRIGGER package_artifacts_no_update_v3
BEFORE UPDATE ON package_artifacts_v3
BEGIN SELECT RAISE(ABORT, 'Package Artifact is immutable'); END;

CREATE TRIGGER package_artifacts_no_delete_v3
BEFORE DELETE ON package_artifacts_v3
BEGIN SELECT RAISE(ABORT, 'Package Artifact is immutable'); END;

CREATE TRIGGER package_artifacts_no_late_insert_v3
BEFORE INSERT ON package_artifacts_v3
WHEN EXISTS (SELECT 1 FROM packages_v3 WHERE package_id = NEW.package_id)
BEGIN SELECT RAISE(ABORT, 'Package Artifact cannot be appended'); END;

CREATE UNIQUE INDEX package_artifacts_one_feedback_v3
    ON package_artifacts_v3(package_id)
    WHERE role = 'feedback';

CREATE UNIQUE INDEX package_artifacts_one_uncooked_v3
    ON package_artifacts_v3(package_id)
    WHERE role = 'uncooked';

CREATE TABLE feedback_deliveries_v3 (
    delivery_id TEXT PRIMARY KEY NOT NULL,
    request_id TEXT NOT NULL UNIQUE,
    session_id TEXT NOT NULL,
    resolution TEXT NOT NULL CHECK (resolution IN ('submitted', 'cancelled')),
    package_id TEXT,
    state TEXT NOT NULL CHECK (state IN ('pending', 'delivered')),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    last_error_code TEXT,
    last_error_at TEXT,
    created_at TEXT NOT NULL CHECK (length(created_at) > 0),
    delivered_at TEXT,
    UNIQUE (session_id, delivery_id),
    FOREIGN KEY (session_id, request_id)
        REFERENCES feedback_requests_v3(session_id, request_id) ON DELETE RESTRICT,
    FOREIGN KEY (request_id, package_id)
        REFERENCES packages_v3(request_id, package_id) ON DELETE RESTRICT,
    CHECK (
        (resolution = 'submitted' AND package_id IS NOT NULL)
        OR
        (resolution = 'cancelled' AND package_id IS NULL)
    ),
    CHECK (
        (state = 'pending' AND delivered_at IS NULL)
        OR
        (state = 'delivered'
            AND delivered_at IS NOT NULL
            AND length(delivered_at) > 0)
    ),
    CHECK (
        (last_error_code IS NULL AND last_error_at IS NULL)
        OR
        (last_error_code IS NOT NULL
            AND length(last_error_code) > 0
            AND last_error_at IS NOT NULL
            AND length(last_error_at) > 0)
    )
);

CREATE INDEX feedback_deliveries_pending_v3
    ON feedback_deliveries_v3(session_id, created_at, delivery_id)
    WHERE state = 'pending';

CREATE TRIGGER feedback_deliveries_match_resolution_v3
BEFORE INSERT ON feedback_deliveries_v3
WHEN NOT EXISTS (
    SELECT 1
    FROM feedback_requests_v3
    WHERE request_id = NEW.request_id
      AND session_id = NEW.session_id
      AND resolution = NEW.resolution
      AND response_package_id IS NEW.package_id
)
BEGIN
    SELECT RAISE(ABORT, 'Feedback Delivery must match the Request resolution');
END;

CREATE TRIGGER feedback_deliveries_identity_is_immutable_v3
BEFORE UPDATE OF request_id, session_id, resolution, package_id
ON feedback_deliveries_v3
BEGIN
    SELECT RAISE(ABORT, 'Feedback Delivery identity is immutable');
END;

CREATE TRIGGER feedback_deliveries_delivered_is_terminal_v3
BEFORE UPDATE ON feedback_deliveries_v3
WHEN OLD.state = 'delivered'
BEGIN
    SELECT RAISE(ABORT, 'Delivered Feedback Delivery is terminal');
END;

CREATE TABLE agent_work_v3 (
    work_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (
        kind IN ('launch_prompt', 'steering_prompt', 'feedback_resume')
    ),
    source_submission_id TEXT UNIQUE,
    source_delivery_id TEXT UNIQUE,
    payload_digest TEXT NOT NULL CHECK (
        length(payload_digest) = 71
        AND substr(payload_digest, 1, 7) = 'sha256:'
        AND substr(payload_digest, 8) NOT GLOB '*[^0-9a-f]*'
    ),
    state TEXT NOT NULL CHECK (state IN ('pending', 'claimed', 'completed')),
    lease_token TEXT,
    lease_until TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    last_error_code TEXT,
    last_error_at TEXT,
    created_at TEXT NOT NULL CHECK (length(created_at) > 0),
    completed_at TEXT,
    FOREIGN KEY (session_id, source_submission_id)
        REFERENCES ramble_submissions_v3(session_id, submission_id) ON DELETE RESTRICT,
    FOREIGN KEY (session_id, source_delivery_id)
        REFERENCES feedback_deliveries_v3(session_id, delivery_id) ON DELETE RESTRICT,
    CHECK (
        (kind IN ('launch_prompt', 'steering_prompt')
            AND source_submission_id IS NOT NULL
            AND source_delivery_id IS NULL)
        OR
        (kind = 'feedback_resume'
            AND source_submission_id IS NULL
            AND source_delivery_id IS NOT NULL)
    ),
    CHECK (
        (state = 'pending'
            AND lease_token IS NULL
            AND lease_until IS NULL
            AND completed_at IS NULL)
        OR
        (state = 'claimed'
            AND lease_token IS NOT NULL
            AND length(lease_token) > 0
            AND lease_until IS NOT NULL
            AND length(lease_until) > 0
            AND completed_at IS NULL)
        OR
        (state = 'completed'
            AND lease_token IS NOT NULL
            AND length(lease_token) > 0
            AND lease_until IS NULL
            AND completed_at IS NOT NULL
            AND length(completed_at) > 0)
    ),
    CHECK (
        (last_error_code IS NULL AND last_error_at IS NULL)
        OR
        (last_error_code IS NOT NULL
            AND length(last_error_code) > 0
            AND last_error_at IS NOT NULL
            AND length(last_error_at) > 0)
    ),
    CHECK (state != 'completed' OR last_error_code IS NULL)
);

CREATE INDEX agent_work_claimable_v3
    ON agent_work_v3(session_id, state, lease_until, created_at, work_id);

CREATE TRIGGER agent_work_requires_managed_session_v3
BEFORE INSERT ON agent_work_v3
WHEN NOT EXISTS (
    SELECT 1
    FROM sessions_v3
    WHERE session_id = NEW.session_id AND session_kind = 'managed'
)
BEGIN
    SELECT RAISE(ABORT, 'Agent work requires a Managed Session');
END;

CREATE TRIGGER agent_work_identity_is_immutable_v3
BEFORE UPDATE OF session_id, kind, source_submission_id, source_delivery_id, payload_digest
ON agent_work_v3
BEGIN
    SELECT RAISE(ABORT, 'Agent work identity is immutable');
END;

CREATE TRIGGER agent_work_completed_is_terminal_v3
BEFORE UPDATE ON agent_work_v3
WHEN OLD.state = 'completed'
BEGIN
    SELECT RAISE(ABORT, 'Completed Agent work is terminal');
END;

CREATE TRIGGER agent_work_matches_submission_intent_v3
BEFORE INSERT ON agent_work_v3
WHEN NEW.kind IN ('launch_prompt', 'steering_prompt') AND NOT EXISTS (
    SELECT 1
    FROM ramble_submissions_v3
    WHERE submission_id = NEW.source_submission_id
      AND session_id = NEW.session_id
      AND intent = CASE NEW.kind
          WHEN 'launch_prompt' THEN 'launch'
          WHEN 'steering_prompt' THEN 'steering'
      END
)
BEGIN
    SELECT RAISE(ABORT, 'Agent work must match the Ramble Submission intent');
END;

CREATE TRIGGER agent_work_matches_feedback_delivery_v3
BEFORE INSERT ON agent_work_v3
WHEN NEW.kind = 'feedback_resume' AND NOT EXISTS (
    SELECT 1
    FROM feedback_deliveries_v3
    WHERE delivery_id = NEW.source_delivery_id
      AND session_id = NEW.session_id
      AND state = 'pending'
)
BEGIN
    SELECT RAISE(ABORT, 'Feedback resume work requires a pending Feedback Delivery');
END;
