#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
inspector_version="${INSPECTOR_VERSION:-2.0.0}"
temp_root="${TMPDIR:-/tmp}"
verify_dir="$(mktemp -d "${temp_root%/}/rambledesk-inspector.XXXXXX")"
server_pid=""

cleanup() {
  if [[ -n "$server_pid" ]]; then
    kill -INT "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  case "$verify_dir" in
    "${temp_root%/}"/rambledesk-inspector.*) rm -rf -- "$verify_dir" ;;
  esac
}
trap cleanup EXIT

cd "$repo_root"
cargo build -p rambledesk-cli

start_server() {
  target/debug/rambledesk serve \
    --port 0 \
    --token-file "$verify_dir/token" \
    --database-file "$verify_dir/rambledesk.sqlite3" \
    >"$verify_dir/status.json" \
    2>"$verify_dir/server.log" &
  server_pid=$!

  for _ in {1..150}; do
    if [[ -s "$verify_dir/token" ]] &&
      node -e 'JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"))' \
        "$verify_dir/status.json" 2>/dev/null; then
      endpoint="$(
        node -e 'process.stdout.write(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).endpoint)' \
          "$verify_dir/status.json"
      )"
      return
    fi
    if ! kill -0 "$server_pid" 2>/dev/null; then
      echo "RambleDesk MCP server exited before becoming ready" >&2
      sed -n '1,120p' "$verify_dir/server.log" >&2
      exit 1
    fi
    sleep 0.1
  done
  echo "Timed out waiting for RambleDesk MCP server" >&2
  exit 1
}

force_stop_server() {
  kill -KILL "$server_pid"
  wait "$server_pid" 2>/dev/null || true
  server_pid=""
}

start_server
token="$(<"$verify_dir/token")"

unauthorized_status="$(
  curl --silent --output /dev/null --write-out '%{http_code}' \
    --request POST "$endpoint"
)"
if [[ "$unauthorized_status" != "401" ]]; then
  echo "Expected unauthenticated MCP request to return 401, got $unauthorized_status" >&2
  exit 1
fi

pnpm dlx "@modelcontextprotocol/inspector@${inspector_version}" --cli "$endpoint" \
  --method tools/list \
  --header "Authorization: Bearer $token" \
  --format json \
  >"$verify_dir/tools-list.json"

pnpm dlx "@modelcontextprotocol/inspector@${inspector_version}" --cli "$endpoint" \
  --method tools/call \
  --tool-name rambledesk_health \
  --tool-args-json '{}' \
  --header "Authorization: Bearer $token" \
  --format json \
  >"$verify_dir/tool-call.json"

request_id="0195f7e2-5c31-7b5a-8ab7-3c84ea4fc827"
request_args="$(
  node -e 'process.stdout.write(JSON.stringify({
    request_id: process.argv[2],
    agent: "mcp-inspector",
    session_id: "inspector-smoke",
    project: { name: "RambleDesk Inspector smoke", root_path: process.argv[1] },
    what_happened: "The persistent MCP request tools were exercised.",
    actions: [{ id: "verify", instruction: "Verify the durable request survives restart." }],
    context_refs: [{ label: "protocol", uri: "file:///docs/PROTOCOL.md" }]
  }))' "$verify_dir" "$request_id"
)"

pnpm dlx "@modelcontextprotocol/inspector@${inspector_version}" --cli "$endpoint" \
  --method tools/call \
  --tool-name request_feedback \
  --tool-args-json "$request_args" \
  --header "Authorization: Bearer $token" \
  --format json \
  >"$verify_dir/request-feedback.json"

# A successful tool response must already be durable. Simulate a process crash
# before recovery, then reopen the same database and token.
force_stop_server
start_server

pnpm dlx "@modelcontextprotocol/inspector@${inspector_version}" --cli "$endpoint" \
  --method tools/call \
  --tool-name get_feedback \
  --tool-args-json "{\"request_id\":\"$request_id\"}" \
  --header "Authorization: Bearer $token" \
  --format json \
  >"$verify_dir/get-feedback.json"

pnpm dlx "@modelcontextprotocol/inspector@${inspector_version}" --cli "$endpoint" \
  --method tools/call \
  --tool-name cancel_feedback \
  --tool-args-json "{\"request_id\":\"$request_id\",\"reason\":\"Inspector smoke completed.\"}" \
  --header "Authorization: Bearer $token" \
  --format json \
  >"$verify_dir/cancel-feedback.json"

# Cancelled is terminal and must also survive an ungraceful process exit.
force_stop_server
start_server

pnpm dlx "@modelcontextprotocol/inspector@${inspector_version}" --cli "$endpoint" \
  --method tools/call \
  --tool-name get_feedback \
  --tool-args-json "{\"request_id\":\"$request_id\"}" \
  --header "Authorization: Bearer $token" \
  --format json \
  >"$verify_dir/get-after-cancel-restart.json"

# Waiting on an already-terminal request must return immediately after restart.
pnpm dlx "@modelcontextprotocol/inspector@${inspector_version}" --cli "$endpoint" \
  --method tools/call \
  --tool-name wait_for_feedback \
  --tool-args-json "{\"request_id\":\"$request_id\"}" \
  --header "Authorization: Bearer $token" \
  --format json \
  >"$verify_dir/wait-after-cancel-restart.json"

node - \
  "$verify_dir/tools-list.json" \
  "$verify_dir/tool-call.json" \
  "$verify_dir/request-feedback.json" \
  "$verify_dir/get-feedback.json" \
  "$verify_dir/cancel-feedback.json" \
  "$verify_dir/get-after-cancel-restart.json" \
  "$verify_dir/wait-after-cancel-restart.json" \
  "$request_id" <<'NODE'
const fs = require('fs')

const listed = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'))
const called = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'))
const requested = JSON.parse(fs.readFileSync(process.argv[4], 'utf8'))
const fetched = JSON.parse(fs.readFileSync(process.argv[5], 'utf8'))
const cancelled = JSON.parse(fs.readFileSync(process.argv[6], 'utf8'))
const recoveredCancelled = JSON.parse(fs.readFileSync(process.argv[7], 'utf8'))
const waitedCancelled = JSON.parse(fs.readFileSync(process.argv[8], 'utf8'))
const expectedRequestId = process.argv[9]
const tools = listed.result?.tools ?? []
const health = called.result?.structuredContent
const createdRequest = requested.result?.structuredContent
const fetchedRequest = fetched.result?.structuredContent
const cancelledRequest = cancelled.result?.structuredContent
const recoveredCancelledRequest = recoveredCancelled.result?.structuredContent
const waitedCancelledRequest = waitedCancelled.result?.structuredContent

for (const expected of [
  'rambledesk_health',
  'request_feedback',
  'wait_for_feedback',
  'get_feedback',
  'cancel_feedback',
]) {
  if (!tools.some((tool) => tool.name === expected)) {
    throw new Error(`Inspector did not list ${expected}`)
  }
}
if (
  health?.serviceName !== 'rambledesk' ||
  health?.status !== 'ready' ||
  health?.storage !== 'ready'
) {
  throw new Error(`Unexpected health result: ${JSON.stringify(health)}`)
}
if (
  createdRequest?.status !== 'waiting' ||
  createdRequest?.request_id !== expectedRequestId ||
  fetchedRequest?.request_id !== expectedRequestId ||
  fetchedRequest?.status !== 'waiting' ||
  cancelledRequest?.request_id !== expectedRequestId ||
  cancelledRequest?.status !== 'cancelled' ||
  recoveredCancelledRequest?.request_id !== expectedRequestId ||
  recoveredCancelledRequest?.status !== 'cancelled' ||
  waitedCancelledRequest?.request_id !== expectedRequestId ||
  waitedCancelledRequest?.status !== 'cancelled' ||
  waitedCancelledRequest?.execution_mode !== 'wait' ||
  waitedCancelledRequest?.feedback_package !== null
) {
  throw new Error(
    `Unexpected feedback lifecycle: ${JSON.stringify({
      createdRequest,
      fetchedRequest,
      cancelledRequest,
      recoveredCancelledRequest,
      waitedCancelledRequest,
    })}`,
  )
}

process.stdout.write(
  `${JSON.stringify({
    inspector: 'passed',
    feedbackLifecycle: 'waiting -> crash recovery -> cancelled -> crash recovery -> blocking wait return',
    endpointPath: '/mcp',
    protocolVersion: health.protocolVersion,
    clientSupportsTasks: health.clientSupportsTasks,
    unauthorizedStatus: 401,
  }, null, 2)}\n`,
)
NODE
