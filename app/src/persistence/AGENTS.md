# app/src/persistence — SQLite runtime, schema, queries

App-side persistence. Migrations are next door at `crates/persistence/migrations/`. **Read `README.md` in this directory first** — it's the canonical migration playbook.

## Files

- `mod.rs` — module surface
- `sqlite.rs` — query / runtime layer (~149K, big)
- `sqlite_tests.rs` — round-trip tests; rerun on any schema change
- `schema.rs` — **generated** by `diesel migration run`. Do **not** edit by hand.
- `schema.patch` — manually-maintained delta applied on top of `schema.rs`. Regenerate with `git diff -U6 > app/src/persistence/schema.patch`.
- `agent.rs`, `block_list.rs`, `cloud_objects.rs`, `commands.rs`, `cloud_object_tests.rs`, `testing.rs` — domain-specific persistence modules
- `README.md` — the playbook (one-time setup, generate migration, run, regenerate schema, schema.patch workflow)

## Schema change workflow (must follow)

1. Use **our forked `diesel_cli`** (installed by `./script/bootstrap`).
2. `diesel migration generate <name>` in `crates/persistence/`. Edit `up.sql` and `down.sql`.
3. Run against the dev DB:
   ```bash
   diesel migration run --database-url="$DEV_DB"
   ```
   This regenerates `schema.rs` automatically. **Do not** hand-edit it.
4. If schema needs additional Rust-side overrides, edit `schema.patch`:
   ```bash
   git diff -U6 > app/src/persistence/schema.patch
   ```
5. Update `sqlite.rs` / domain modules to use the new schema.
6. Run `cargo nextest run -p warp persistence::sqlite_tests` (round-trip).

The `up.sql` runs in production inside a single transaction at app startup. Make it **idempotent** and safe against partially-applied earlier migrations.

## Schema style

- `id` for integer primary keys, unless a more descriptive name is needed.
- Plural table names; singular Rust struct names.
- Foreign keys: `<table_singular>_id` (e.g., `bars.foo_id` → `foos.id`).

## Don't

- Don't edit `schema.rs` by hand.
- Don't ship a migration without a working `down.sql`.
- Don't use SQL features that the bundled SQLite (an older version, vendored via our fork) doesn't support.
- Don't use upstream `diesel_cli`. Use the fork.

## Cross-references

- Migrations live in: `crates/persistence/migrations/` (~110 migrations, 2021-10 → present).
- Cloud sync of preferences/data: `app/src/settings/cloud_preferences*.rs` and `app/src/drive/`.
- Round-trip and snapshot tests: `sqlite_tests.rs` here, plus integration fixtures in `crates/integration/tests/data/`.
