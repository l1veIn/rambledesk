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

target/debug/rambledesk serve \
  --port 0 \
  --token-file "$verify_dir/token" \
  >"$verify_dir/status.json" \
  2>"$verify_dir/server.log" &
server_pid=$!

for _ in {1..150}; do
  if [[ -s "$verify_dir/token" ]] &&
    node -e 'JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"))' \
      "$verify_dir/status.json" 2>/dev/null; then
    break
  fi
  if ! kill -0 "$server_pid" 2>/dev/null; then
    echo "RambleDesk MCP server exited before becoming ready" >&2
    sed -n '1,120p' "$verify_dir/server.log" >&2
    exit 1
  fi
  sleep 0.1
done

endpoint="$(
  node -e 'process.stdout.write(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).endpoint)' \
    "$verify_dir/status.json"
)"
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

node - "$verify_dir/tools-list.json" "$verify_dir/tool-call.json" <<'NODE'
const fs = require('fs')

const listed = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'))
const called = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'))
const tools = listed.result?.tools ?? []
const health = called.result?.structuredContent

if (!tools.some((tool) => tool.name === 'rambledesk_health')) {
  throw new Error('Inspector did not list rambledesk_health')
}
if (
  health?.serviceName !== 'rambledesk' ||
  health?.status !== 'ready' ||
  health?.storage !== 'not_initialized'
) {
  throw new Error(`Unexpected health result: ${JSON.stringify(health)}`)
}

process.stdout.write(
  `${JSON.stringify({
    inspector: 'passed',
    endpointPath: '/mcp',
    protocolVersion: health.protocolVersion,
    clientSupportsTasks: health.clientSupportsTasks,
    unauthorizedStatus: 401,
  }, null, 2)}\n`,
)
NODE
