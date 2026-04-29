# crates/persistence — Diesel migrations

This crate hosts **Diesel migrations**. ~110 migrations from 2021-10 onward. The corresponding generated schema lives in `app/src/persistence/schema.rs` (with a manual delta in `schema.patch`); the runtime SQLite code lives under `app/src/persistence/`.

## Layout

```
crates/persistence/
├── migrations/                         # YYYY-MM-DD-HHMMSS_<name>/{up,down}.sql
├── src/                                # tiny runtime surface
├── schema.patch                        # 562B — see app/src/persistence/README.md
├── build.rs
└── Cargo.toml
```

## Adding a migration (cheat sheet)

Authoritative reference: `app/src/persistence/README.md`. Short version:

1. Run `./script/bootstrap` once. It installs **our fork** of `diesel_cli` (which bundles SQLite). Do NOT use upstream Diesel CLI.
2. Generate the migration:
   ```bash
   diesel migration generate <descriptive_name>
   ```
   This creates `migrations/<timestamp>_<name>/{up,down}.sql`.
3. Run it locally against your dev DB (path is platform-specific; macOS shown):
   ```bash
   diesel migration run --database-url="/Users/$USER/Library/Application Support/dev.warp.Warp-Local/warp.sqlite"
   ```
4. **Do not edit `app/src/persistence/schema.rs` manually.** The migration run regenerates it.
5. If you need overrides on top of the generated schema, edit `schema.patch` (regenerate via `git diff -U6 > app/src/persistence/schema.patch`).

Revert / redo while iterating:
```bash
diesel migration revert --database-url="..."
diesel migration redo   --database-url="..."
```

## Schema style conventions

- Integer primary keys are named `id` unless something more descriptive is warranted.
- Plural table names; singular Rust struct names.
- Foreign keys: `<table_singular>_id` (e.g., `bars.foo_id` references `foos.id`).

## What runs in production

- The SQLite database is a **single file on the user's machine**; SQLite is bundled (C functions linked into the app), not a separate process.
- On startup the app upgrades the schema to the latest version inside one transaction. Migrations therefore must be **idempotent and safe to re-run** in the face of partial earlier failures.

There's still a known TODO around CI and remediation of failed migrations (see `app/src/persistence/README.md`).

## Don't

- Don't hand-edit `schema.rs`.
- Don't ship a migration without a working `down.sql`.
- Don't use SQL features that older bundled SQLite versions don't support — keep an eye on `Cargo.lock`'s SQLite version when in doubt.
