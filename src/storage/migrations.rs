//! Schema migration runner.
//!
//! Migrations are numbered SQL scripts embedded at compile time via
//! [`include_str!`].  On every database open, [`run`] applies any migrations
//! whose version number is greater than the current `schema_migrations` max.
//!
//! ## Adding a migration
//!
//! 1. Create `src/storage/sql/NNN_description.sql` (e.g. `002_add_index.sql`).
//! 2. Append `(N, include_str!("sql/NNN_description.sql"))` to [`MIGRATIONS`].
//!
//! **Never edit an existing migration file.** Only append new entries.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::Connection;

/// All schema migrations in ascending version order.
///
/// Each tuple is `(version, sql)`.  Version numbers must be contiguous and
/// start at 1.  The SQL is applied in a single `execute_batch` call, so each
/// file may contain multiple statements separated by semicolons.
pub(super) const MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("sql/001_initial.sql")),
    (2, include_str!("sql/002_commit_messages.sql")),
];

/// Apply all pending migrations to `conn`.
///
/// Creates the `schema_migrations` tracking table if it does not yet exist,
/// then runs every migration whose version exceeds the current maximum.
///
/// # Errors
///
/// Returns an error if any migration SQL fails to execute or if the migrations
/// table cannot be created or queried.
pub(super) fn run(conn: &Connection) -> Result<()> {
    // Bootstrap the migrations table on a fresh database.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            applied_at TEXT    NOT NULL
        );",
    )
    .context("Failed to create schema_migrations table")?;

    let current: u32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    for &(version, sql) in MIGRATIONS {
        if version > current {
            conn.execute_batch(sql)
                .with_context(|| format!("Migration {version} failed"))?;
            conn.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                rusqlite::params![version, Utc::now().to_rfc3339()],
            )
            .with_context(|| format!("Failed to record migration {version}"))?;
        }
    }

    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn in_memory() -> Connection {
        Connection::open_in_memory().expect("in-memory db")
    }

    #[test]
    fn migrations_apply_cleanly() {
        let conn = in_memory();
        run(&conn).expect("migrations should succeed");
    }

    #[test]
    fn migrations_are_idempotent() {
        let conn = in_memory();
        run(&conn).expect("first run");
        run(&conn).expect("second run should be a no-op");
    }

    #[test]
    fn expected_tables_exist_after_migration() {
        let conn = in_memory();
        run(&conn).expect("migrations");

        let expected = [
            "schema_migrations",
            "repos",
            "workspaces",
            "stacks",
            "stack_branches",
            "command_runs",
            "branch_events",
            "commit_records",
            "workspace_events",
            "stack_events",
            "commit_messages",
        ];

        for table in &expected {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    rusqlite::params![table],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            assert_eq!(count, 1, "table '{table}' should exist after migration");
        }
    }

    #[test]
    fn schema_version_recorded() {
        let conn = in_memory();
        run(&conn).expect("migrations");

        let max_version: u32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .expect("query schema_migrations");

        assert_eq!(
            max_version,
            MIGRATIONS.last().map(|(v, _)| *v).unwrap_or(0),
            "recorded version should match last migration"
        );
    }
}
