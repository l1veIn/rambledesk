#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
generated="$root/apps/desktop/src/lib/generated"
snapshot="$(mktemp -d "${TMPDIR:-/tmp}/rambledesk-contracts.XXXXXX")"

cleanup() {
  rm -rf -- "$snapshot"
}
trap cleanup EXIT

cp "$generated/feedback.ts" "$snapshot/feedback.ts"

cd "$root"
pnpm contracts:generate

diff -u "$snapshot/feedback.ts" "$generated/feedback.ts"
