ALTER TABLE agent_configs ADD COLUMN catalog_id TEXT
    CHECK (catalog_id IS NULL OR length(trim(catalog_id)) > 0);

CREATE INDEX agent_configs_catalog ON agent_configs(catalog_id);

-- Classify historical recipes once. Runtime association uses catalog_id only.
-- Preserve all user launch settings and leave unknown/ambiguous programs custom.
WITH recipes(catalog_id, host_id, command, package) AS (VALUES
    ('claude-acp', 'claude', 'claude-agent-acp', '@agentclientprotocol/claude-agent-acp'),
    ('codex-acp', 'codex', 'codex-acp', '@agentclientprotocol/codex-acp'),
    ('gemini', 'gemini', 'gemini', '@google/gemini-cli'),
    ('openclaw-acp', 'openclaw', 'openclaw', 'openclaw'),
    ('cline', 'cline', 'cline', 'cline'),
    ('codebuddy', 'codebuddy', 'codebuddy', '@tencent-ai/codebuddy-code'),
    ('kimi', 'kimi', 'kimi', '@moonshot-ai/kimi-code'),
    ('pi-acp', 'pi', 'pi-acp', 'pi-acp'),
    ('grok', 'grok', 'grok', '@xai-official/grok'),
    ('deepseek-acp', 'dsh', 'deepseek-acp', 'deepseek-acp'),
    ('dsh', 'dsh', 'dsh', '@deepseek-ai/dsh'),
    ('qoder', 'qoder', 'qoder', '@qoder-ai/qodercli'),
    ('opencode', 'opencode', 'opencode', NULL),
    ('cursor', 'cursor', 'cursor-agent', NULL),
    ('hermes', 'hermes', 'hermes', NULL),
    ('antigravity', 'antigravity', 'agy_acp_server', NULL)
), normalized AS (
    SELECT id, host_id, args_json,
        CASE WHEN substr(lower(command), -4) IN ('.cmd', '.exe', '.bat')
            THEN substr(lower(replace(command, '\', '/')), 1, length(command) - 4)
            ELSE lower(replace(command, '\', '/')) END AS command
    FROM agent_configs
), candidates AS (
    SELECT config.id, recipe.catalog_id
    FROM normalized config JOIN recipes recipe ON config.host_id = recipe.host_id
    WHERE config.command = recipe.command OR config.command GLOB '*/' || recipe.command
        OR (recipe.package IS NOT NULL AND EXISTS (
            SELECT 1 FROM json_each(config.args_json) arg
            WHERE arg.type = 'text' AND instr(lower(replace(arg.value, '\', '/')),
                '/node_modules/' || recipe.package || '/') > 0
        ))
), unique_matches AS (
    SELECT id, min(catalog_id) catalog_id FROM candidates GROUP BY id HAVING count(DISTINCT catalog_id) = 1
)
UPDATE agent_configs SET catalog_id = (SELECT catalog_id FROM unique_matches WHERE unique_matches.id = agent_configs.id)
WHERE id IN (SELECT id FROM unique_matches);
