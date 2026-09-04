CREATE TABLE agent_configs (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    host_id TEXT NOT NULL CHECK (length(trim(host_id)) > 0),
    protocol TEXT NOT NULL CHECK (protocol = 'acp'),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    command TEXT NOT NULL CHECK (length(trim(command)) > 0),
    args_json TEXT NOT NULL CHECK (json_valid(args_json) AND json_type(args_json) = 'array'),
    env_json TEXT NOT NULL CHECK (json_valid(env_json) AND json_type(env_json) = 'object'),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- No row means externally managed. Existing correlation identities remain intact.
CREATE TABLE managed_sessions (
    session_id TEXT PRIMARY KEY NOT NULL REFERENCES host_sessions(id) ON DELETE CASCADE,
    protocol TEXT NOT NULL CHECK (protocol = 'acp'),
    agent_config_id TEXT NOT NULL REFERENCES agent_configs(id) ON DELETE RESTRICT,
    cwd TEXT NOT NULL CHECK (length(trim(cwd)) > 0),
    remote_session_id TEXT CHECK (remote_session_id IS NULL OR length(trim(remote_session_id)) > 0)
);

CREATE INDEX managed_sessions_agent_config ON managed_sessions(agent_config_id);
