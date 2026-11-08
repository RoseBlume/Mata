# mata-sqlite — pure Rust SQLite file engine (StudyForge subset)

This implementation is **100% Rust** with **no dependencies**, **no FFI** and
`#![forbid(unsafe_code)]`. It replaces `libsqlite3` for the SQL operations used
by the native StudyForge app. It is **not a full SQLite replacement**.

## Supported in this release

- Read ordinary SQLite 3 rowid b-tree tables, records, overflow pages and SQLite
  `sqlite_master` table definitions (UTF-8/UTF-16 reads).
- Write standard SQLite format 3 files with 4096-byte pages via atomic shadow
  files, single Mata-process writer sidecar lock, primary key uniqueness,
  `INSERT`, `INSERT OR IGNORE`, `INSERT OR REPLACE`, `UPDATE`, `DELETE`.
- Parse and execute basic `SELECT`, `COUNT`, `MAX`, `WHERE`, `ORDER BY`,
  `LIMIT`, and StudyForge's fixed join patterns.
- Foreign-key **insert checks**, recursive `ON DELETE CASCADE` and transaction
  rollback when dropped without committing.
- `Connection`, `Transaction`, `Statement`, `params!`, `query_map`,
  `query_row`, typed row values and optional queries.

## Known limits — read this before using important data

This implementation is a **subset engine**. It does **not** implement the
complete SQLite VDBE, general SQL joins, query planner, WAL writing, incremental
page updates, SQLite's native multi-process locks, triggers, views, virtual
or WITHOUT ROWID tables, generated columns, or full constraint semantics.
`CREATE INDEX` statements are accepted for app schema compatibility but not
materialized: the writer rebuilds rowid tables and does not preserve separate
index structures. Existing indexes should be recreated by a full SQLite engine
if the file is later used outside Mata.

The engine refuses non-empty `-wal` or `-shm` sidecars and WAL-mode database
headers. **Never manually delete a WAL file containing uncheckpointed data.**
Checkpoint an old database with your previous SQLite backend while it is
fully closed, turn off WAL, and keep a backup before opening it here.

The Mata sidecar `-mata-lock` protects against concurrent instances of this
engine. Other SQLite processes don't respect it, so close other DB clients.
A crash can leave a stale lock that you must inspect before removing.

Use this engine only on disposable/test databases until the complete SQLite
conformance tests, recovery behavior and stress testing have been validated.

## Check

```sh
cargo test -p mata-sqlite
cargo check -p mata-sqlite -p studyforge
```

The tests include a genuine SQLite 3 fixture created with Python's `sqlite3`
module and Rust-only round-trip, uniqueness, rollback, FK-cascade and WAL
refusal tests. Python is **not** a runtime dependency of `mata-sqlite`.
