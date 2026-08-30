# RambleDesk v2 → v3 migration tool

This is an explicit, disposable migration tool. It is not linked into the
RambleDesk desktop runtime and never calls `SqliteFeedbackStore::connect`.

## Current scope

Only the read-only `inspect` command exists in this phase:

```text
cargo run --manifest-path tools/migrate-v2-to-v3/Cargo.toml -- \
  inspect --source-db /path/to/feedback.sqlite3
```

`inspect` opens the legacy database with SQLite `read_only` and `immutable`
enabled, sets `query_only`, rejects a non-empty WAL sidecar, reads legacy
Package files without changing them, and writes a stable JSON classification
report to stdout.

The command currently does **not** implement `dry-run`, `execute`, `verify`,
backup creation, target database writes, Artifact Store writes, or application
configuration changes. Those commands must be added as real operations in
later phases; they must not be simulated by `inspect` output.

Run RambleDesk's full-exit action before inspection. Immutable SQLite access
assumes the source database and its Package library cannot change while the
command is running.
