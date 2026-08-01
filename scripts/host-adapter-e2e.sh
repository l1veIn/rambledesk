#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
CODEX_BIN=${RAMBLEDESK_CODEX_BIN:-/Applications/ChatGPT.app/Contents/Resources/codex}
OPENCODE_BIN=${RAMBLEDESK_OPENCODE_BIN:-$HOME/.opencode/bin/opencode}
CLAUDE_MODEL_ARGS=()
if [[ -n "${RAMBLEDESK_CLAUDE_MODEL:-}" ]]; then
  CLAUDE_MODEL_ARGS=(--model "$RAMBLEDESK_CLAUDE_MODEL")
fi

need() {
  command -v "$1" >/dev/null || {
    echo "missing required command: $1" >&2
    exit 1
  }
}

lower_uuid() {
  uuidgen | tr '[:upper:]' '[:lower:]'
}

assert_contains() {
  local file=$1
  local pattern=$2
  if ! rg -q "$pattern" "$file"; then
    echo "expected $file to contain $pattern" >&2
    sed -n '1,120p' "$file" >&2
    exit 1
  fi
}

run_claude() {
  command -v claude >/dev/null || {
    echo "skip claude: binary not found"
    return 0
  }
  local dir sid run_id
  dir=$(mktemp -d /tmp/rambledesk-claude-e2e.XXXXXX)
  sid=$(lower_uuid)
  (
    cd "$dir"
    claude "${CLAUDE_MODEL_ARGS[@]}" --session-id "$sid" -p --output-format json \
      "RambleDesk adapter E2E probe. Do not use tools. Reply exactly: RD_E2E_CLAUDE_READY" \
      > first.json
    claude "${CLAUDE_MODEL_ARGS[@]}" --resume "$sid" -p --output-format json \
      "RambleDesk adapter E2E resume probe. Do not use tools. Reply exactly: RD_E2E_CLAUDE_RESUMED" \
      > second.json
  )
  [[ "$(jq -r '.session_id // empty' "$dir/second.json")" == "$sid" ]]
  assert_contains "$dir/first.json" "RD_E2E_CLAUDE_READY"
  assert_contains "$dir/second.json" "RD_E2E_CLAUDE_RESUMED"

  run_id=$(
    cd "$dir"
    claude "${CLAUDE_MODEL_ARGS[@]}" --resume "$sid" --background \
      "RambleDesk adapter background wake probe. Do not edit files. Reply exactly: RD_E2E_CLAUDE_BACKGROUND" |
      awk '/backgrounded ·/ {print $3}'
  )
  for _ in $(seq 1 30); do
    claude logs "$run_id" > "$dir/background.log" || true
    if rg -q "RD_E2E_CLAUDE_BACKGROUND" "$dir/background.log"; then
      break
    fi
    sleep 1
  done
  claude stop "$run_id" >/dev/null 2>&1 || true
  assert_contains "$dir/background.log" "RD_E2E_CLAUDE_BACKGROUND"
  echo "claude ok: session=$sid dir=$dir"
}

run_codex() {
  [[ -x "$CODEX_BIN" ]] || command -v codex >/dev/null || {
    echo "skip codex: binary not found"
    return 0
  }
  local dir sid
  dir=$(mktemp -d /tmp/rambledesk-codex-e2e.XXXXXX)
  (
    cd "$dir"
    "$CODEX_BIN" exec --json --skip-git-repo-check \
      "RambleDesk adapter E2E probe. Do not edit files. Reply exactly: RD_E2E_CODEX_READY" \
      > first.jsonl
  )
  sid=$(jq -r 'select(.type == "thread.started") | .thread_id' "$dir/first.jsonl" | head -1)
  [[ -n "$sid" ]]
  (
    cd "$ROOT_DIR"
    "$CODEX_BIN" exec resume "$sid" \
      "RambleDesk adapter cross-cwd resume probe. Do not edit files. Reply exactly: RD_E2E_CODEX_CROSS_CWD" \
      --json > "$dir/second.jsonl"
  )
  assert_contains "$dir/first.jsonl" "RD_E2E_CODEX_READY"
  assert_contains "$dir/second.jsonl" "RD_E2E_CODEX_CROSS_CWD"
  echo "codex ok: session=$sid dir=$dir"
}

run_pi() {
  command -v pi >/dev/null || {
    echo "skip pi: binary not found"
    return 0
  }
  local dir sessions sid
  dir=$(mktemp -d /tmp/rambledesk-pi-e2e.XXXXXX)
  sessions="$dir/sessions"
  sid=$(lower_uuid)
  mkdir -p "$sessions" "$dir/project"
  (
    cd "$dir/project"
    pi --session-dir "$sessions" --session-id "$sid" --print --mode json \
      "RambleDesk adapter E2E probe. Do not edit files. Reply exactly: RD_E2E_PI_READY" \
      > "$dir/first.json"
    pi --session-dir "$sessions" --session "$sid" --print --mode json \
      "RambleDesk adapter cwd-fixed probe. Do not edit files. Reply exactly: RD_E2E_PI_CWD_FIXED" \
      > "$dir/second.json"
  )
  assert_contains "$dir/first.json" "RD_E2E_PI_READY"
  assert_contains "$dir/second.json" "RD_E2E_PI_CWD_FIXED"
  echo "pi ok: session=$sid dir=$dir"
}

run_opencode() {
  [[ -x "$OPENCODE_BIN" ]] || command -v opencode >/dev/null || {
    echo "skip opencode: binary not found"
    return 0
  }
  local dir sid
  dir=$(mktemp -d /tmp/rambledesk-opencode-e2e.XXXXXX)
  mkdir -p "$dir/project"
  "$OPENCODE_BIN" run --format json --dir "$dir/project" \
    "RambleDesk adapter E2E probe. Do not edit files. Reply exactly: RD_E2E_OPENCODE_READY" \
    > "$dir/first.jsonl"
  sid=$(jq -r 'select(.sessionID != null) | .sessionID' "$dir/first.jsonl" | head -1)
  [[ -n "$sid" ]]
  (
    cd "$ROOT_DIR"
    "$OPENCODE_BIN" run --format json --dir "$dir/project" --session "$sid" \
      "RambleDesk adapter dir-fixed resume probe. Do not edit files. Reply exactly: RD_E2E_OPENCODE_WITH_DIR" \
      > "$dir/second.jsonl"
  )
  assert_contains "$dir/first.jsonl" "RD_E2E_OPENCODE_READY"
  assert_contains "$dir/second.jsonl" "RD_E2E_OPENCODE_WITH_DIR"
  echo "opencode ok: session=$sid dir=$dir"
}

run_claude_mcp_request() {
  [[ "${RAMBLEDESK_E2E_MCP:-}" == "1" ]] || return 0
  need sqlite3
  local dir server_pid endpoint token sid rid project_root
  dir=$(mktemp -d /tmp/rambledesk-claude-mcp-e2e.XXXXXX)
  target/debug/rambledesk serve \
    --port 0 \
    --token-file "$dir/token" \
    --database-file "$dir/rambledesk.sqlite3" \
    --print-token > "$dir/server.json" 2> "$dir/server.log" &
  server_pid=$!
  trap 'kill -INT "$server_pid" 2>/dev/null || true' RETURN
  for _ in $(seq 1 80); do
    [[ -s "$dir/server.json" ]] && break
    sleep 0.25
  done
  endpoint=$(jq -r '.endpoint' "$dir/server.json")
  token=$(jq -r '.accessToken' "$dir/server.json")
  sid=$(lower_uuid)
  rid=$(lower_uuid)
  project_root="$dir/project"
  mkdir -p "$project_root"
  jq -n --arg url "$endpoint" --arg auth "Bearer $token" \
    '{mcpServers:{rambledesk:{type:"http",url:$url,headers:{Authorization:$auth},env:{RAMBLEDESK_HOST:"claude"}}}}' \
    > "$dir/claude-mcp.json"
  (
    cd "$project_root"
    claude "${CLAUDE_MODEL_ARGS[@]}" --session-id "$sid" -p --output-format json \
      --mcp-config "$dir/claude-mcp.json" --strict-mcp-config \
      --allowedTools mcp__rambledesk__request_feedback \
      --permission-mode dontAsk \
      "Call request_feedback exactly once with request_id '$rid', agent 'claude', session_id '$sid', project.name 'RambleDesk Claude MCP E2E', project.root_path '$project_root', title 'Claude MCP E2E', what_happened 'Testing real Claude MCP request creation.', actions [{id:'continue', instruction:'Continue after feedback is submitted.'}], context_refs []. Then reply exactly: RD_E2E_CLAUDE_MCP_REQUESTED $rid" \
      > "$dir/claude.json"
  )
  sqlite3 "$dir/rambledesk.sqlite3" \
    "SELECT r.id || '|' || s.agent || '|' || s.external_session_id FROM feedback_requests r JOIN agent_sessions s ON s.id = r.session_id WHERE r.id = '$rid';" \
    > "$dir/stored.txt"
  assert_contains "$dir/stored.txt" "$rid|claude|$sid"
  echo "claude mcp request ok: request=$rid session=$sid dir=$dir"
}

run_claude_full_loop() {
  [[ "${RAMBLEDESK_E2E_MCP:-}" == "1" ]] || {
    echo "set RAMBLEDESK_E2E_MCP=1 to run claude-full" >&2
    exit 1
  }
  need sqlite3
  local dir server_pid endpoint token sid rid project_root pkg sha now
  dir=$(mktemp -d /tmp/rambledesk-claude-full-e2e.XXXXXX)
  target/debug/rambledesk serve \
    --port 0 \
    --token-file "$dir/token" \
    --database-file "$dir/rambledesk.sqlite3" \
    --print-token > "$dir/server.json" 2> "$dir/server.log" &
  server_pid=$!
  trap 'kill -INT "$server_pid" 2>/dev/null || true' RETURN
  for _ in $(seq 1 80); do
    [[ -s "$dir/server.json" ]] && break
    sleep 0.25
  done
  endpoint=$(jq -r '.endpoint' "$dir/server.json")
  token=$(jq -r '.accessToken' "$dir/server.json")
  sid=$(lower_uuid)
  rid=$(lower_uuid)
  project_root="$dir/project"
  mkdir -p "$project_root"
  jq -n --arg url "$endpoint" --arg auth "Bearer $token" \
    '{mcpServers:{rambledesk:{type:"http",url:$url,headers:{Authorization:$auth},env:{RAMBLEDESK_HOST:"claude"}}}}' \
    > "$dir/claude-mcp.json"
  (
    cd "$project_root"
    claude "${CLAUDE_MODEL_ARGS[@]}" --session-id "$sid" -p --output-format json \
      --mcp-config "$dir/claude-mcp.json" --strict-mcp-config \
      --allowedTools mcp__rambledesk__request_feedback \
      --permission-mode dontAsk \
      "Call request_feedback exactly once with request_id '$rid', agent 'claude', session_id '$sid', project.name 'RambleDesk Full Claude E2E', project.root_path '$project_root', title 'Full Claude E2E', what_happened 'Testing full request and get_feedback loop.', actions [{id:'continue', instruction:'Continue after feedback is submitted.'}], context_refs []. Then reply exactly: RD_E2E_FULL_REQUESTED $rid" \
      > "$dir/request.json"
  )
  pkg="$project_root/.rambledesk/feedback/$rid"
  mkdir -p "$pkg"
  printf '%s\n' "RD_E2E_PACKAGE_BODY" > "$pkg/feedback.md"
  printf '{"request_id":"%s","attachments":[]}\n' "$rid" > "$pkg/manifest.json"
  sha=$(shasum -a 256 "$pkg/manifest.json" | awk '{print $1}')
  now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  sqlite3 "$dir/rambledesk.sqlite3" \
    "INSERT INTO feedback_results (request_id, package_uri, directory_path, markdown_path, manifest_path, manifest_sha256, published_at) VALUES ('$rid', 'file://$pkg', '$pkg', '$pkg/feedback.md', '$pkg/manifest.json', '$sha', '$now'); UPDATE feedback_requests SET status = 'completed', revision = revision + 1, updated_at = '$now', completed_at = '$now' WHERE id = '$rid';"
  (
    cd "$project_root"
    claude "${CLAUDE_MODEL_ARGS[@]}" --resume "$sid" -p --output-format json \
      --mcp-config "$dir/claude-mcp.json" --strict-mcp-config \
      --allowedTools mcp__rambledesk__get_feedback \
      --permission-mode dontAsk \
      "RambleDesk feedback request $rid is completed. Call get_feedback with this request_id. Verify the feedback_package markdown contains RD_E2E_PACKAGE_BODY. Then reply exactly: RD_E2E_CLAUDE_GET_FEEDBACK_OK" \
      > "$dir/resume.json"
  )
  assert_contains "$dir/request.json" "RD_E2E_FULL_REQUESTED"
  assert_contains "$dir/resume.json" "RD_E2E_CLAUDE_GET_FEEDBACK_OK"
  sqlite3 "$dir/rambledesk.sqlite3" \
    "SELECT r.status || '|' || s.agent || '|' || s.external_session_id FROM feedback_requests r JOIN agent_sessions s ON s.id = r.session_id WHERE r.id = '$rid';" \
    > "$dir/stored.txt"
  assert_contains "$dir/stored.txt" "completed|claude|$sid"
  echo "claude full loop ok: request=$rid session=$sid dir=$dir"
}

need jq
need rg

hosts=("$@")
if [[ ${#hosts[@]} -eq 0 ]]; then
  hosts=(claude codex pi opencode)
fi

for host in "${hosts[@]}"; do
  case "$host" in
    claude) run_claude ;;
    codex) run_codex ;;
    pi) run_pi ;;
    opencode) run_opencode ;;
    claude-mcp) run_claude_mcp_request ;;
    claude-full) run_claude_full_loop ;;
    all)
      run_claude
      run_codex
      run_pi
      run_opencode
      run_claude_mcp_request
      ;;
    *)
      echo "unknown host: $host" >&2
      exit 1
      ;;
  esac
done
