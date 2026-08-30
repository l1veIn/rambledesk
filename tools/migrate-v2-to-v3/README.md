# RambleDesk v2 → v3 migration tool

This is an explicit, disposable migration tool. It is not linked into the
RambleDesk desktop runtime and never calls `SqliteFeedbackStore::connect`.

## Commands

`inspect` only classifies the legacy source and never creates a target:

```text
cargo run --manifest-path tools/migrate-v2-to-v3/Cargo.toml -- \
  inspect --source-db /path/to/feedback.sqlite3
```

It opens SQLite with `read_only`, `immutable`, and `query_only`, rejects a
non-empty WAL sidecar, and reads only validated legacy Package entries.

`migrate --dry-run` performs the complete conversion in a private temporary
root, installs the real v3 schema, writes content-addressed objects, and runs
the same integrity verification used by execution. The temporary root is then
discarded; `--target-root` must be absent and remains absent.

```text
cargo run --manifest-path tools/migrate-v2-to-v3/Cargo.toml -- \
  migrate --dry-run \
  --source-db /path/to/feedback.sqlite3 \
  --target-root /path/to/new-rambledesk-v3
```

`migrate --execute` requires a nonexistent target root. It builds everything
in a private sibling staging root, checks the source again, verifies the full
result, then publishes the whole root with an atomic no-replace rename. A
second execution refuses to replace the first result.

```text
cargo run --manifest-path tools/migrate-v2-to-v3/Cargo.toml -- \
  migrate --execute \
  --source-db /path/to/feedback.sqlite3 \
  --target-root /path/to/new-rambledesk-v3
```

`verify` rechecks the v3 schema, foreign keys, Package digests, Artifact Store,
and backup objects without modifying the target:

```text
cargo run --manifest-path tools/migrate-v2-to-v3/Cargo.toml -- \
  verify --target-root /path/to/new-rambledesk-v3
```

## Fixed target layout

```text
new-rambledesk-v3/
├── rambledesk-v3.sqlite3
├── library/artifacts/sha256/...
├── backup/
│   ├── source.sqlite3
│   └── legacy-library/
│       ├── index.json
│       └── objects/sha256/...
└── reports/
    ├── migration-report.json
    └── migration-report.md
```

The entire backup tree is made read-only before publication. Its index records
the original path only as backup provenance; absolute legacy paths are never
written to v3 business tables or Package manifests.

The reader materializes only validated, explicitly referenced legacy files; it
never recursively copies a database-provided directory. Individual Artifacts
are capped at 20 MiB, a Feedback Request or legacy Package at 60 MiB, and a
Package manifest at 1 MiB/128 entries. The disposable tool also enforces a
256 MiB aggregate in-memory Artifact budget across migration and backup facts.

## Lossy mappings

- `in_progress` becomes waiting.
- Missing actions receive the deterministic `review` action.
- Blank actions/context references are dropped, and lists are capped at 20.
- Blank attachment metadata receives deterministic safe fallbacks.
- Cancelled and approval-style requests are dropped.
- Completed requests with unreadable or unsafe Packages are dropped.
- Submitted history is imported with a delivered Delivery and no Agent work.

Every normalization or loss is included in the JSON and Markdown reports.

Run RambleDesk's full-exit action before inspection. Immutable SQLite access
assumes the source database and its Package library cannot change while the
command is running.
