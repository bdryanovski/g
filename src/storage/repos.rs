//! Repository anchor rows.
//!
//! Every per-repo table (`workspaces`, `stacks`, `command_runs`, …) references
//! `repos(id)` as a foreign key.  [`upsert`] is the single function callers
//! need: it inserts a new repo row on first encounter and updates `last_seen`
//! on every subsequent call, returning the stable row ID either way.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::Connection;

// ─── Types ───────────────────────────────────────────────────────────────────

/// A row from the `repos` table.
#[derive(Debug, Clone)]
pub struct RepoRow {
    /// Primary key.
    pub id: i64,
    /// Absolute filesystem path to the repository (or container) root.
    pub path: String,
    /// Human-readable name derived from the last path component.
    pub name: String,
    /// When this repo was first seen by the tool.
    pub first_seen: DateTime<Utc>,
    /// When this repo was most recently seen.
    pub last_seen: DateTime<Utc>,
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Insert a new repo row for `path`, or update its `last_seen` timestamp if it
/// already exists.  Returns the row's `id` in either case.
///
/// The `name` is derived as the last path component (e.g. `"myapp"` from
/// `"/home/user/myapp"`).
///
/// # Errors
///
/// Returns an error if the SQL fails to execute.
pub fn upsert(conn: &Connection, path: &str) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    let name = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());

    conn.execute(
        "INSERT INTO repos (path, name, first_seen, last_seen)
         VALUES (?1, ?2, ?3, ?3)
         ON CONFLICT(path) DO UPDATE SET last_seen = excluded.last_seen",
        rusqlite::params![path, name, now],
    )
    .context("Failed to upsert repo")?;

    let id: i64 = conn
        .query_row(
            "SELECT id FROM repos WHERE path = ?1",
            rusqlite::params![path],
            |row| row.get(0),
        )
        .context("Failed to fetch repo id after upsert")?;

    Ok(id)
}

/// Load every row from the `repos` table, ordered by `last_seen` descending
/// (most recently active repo first).
///
/// Returns an empty `Vec` when no repos have been recorded yet.
///
/// # Errors
///
/// Returns an error if the SQL fails to execute.
pub fn load_all(conn: &Connection) -> Result<Vec<RepoRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, path, name, first_seen, last_seen
             FROM repos ORDER BY last_seen DESC",
        )
        .context("Failed to prepare repos query")?;

    let rows = stmt
        .query_map([], |row| {
            let first_seen_str: String = row.get(3)?;
            let last_seen_str: String = row.get(4)?;
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                first_seen_str,
                last_seen_str,
            ))
        })
        .context("Failed to query repos")?
        .map(|r| {
            r.map(|(id, path, name, first_str, last_str)| RepoRow {
                id,
                path,
                name,
                first_seen: DateTime::parse_from_rfc3339(&first_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                last_seen: DateTime::parse_from_rfc3339(&last_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to read repo rows")?;

    Ok(rows)
}

/// Return the `id` of the repo row for `path`, or `None` if it has not been
/// seen before.
///
/// # Errors
///
/// Returns an error if the SQL fails to execute.
pub fn find_id(conn: &Connection, path: &str) -> Result<Option<i64>> {
    let mut stmt = conn
        .prepare("SELECT id FROM repos WHERE path = ?1")
        .context("Failed to prepare repo lookup")?;

    let mut rows = stmt
        .query_map(rusqlite::params![path], |row| row.get(0))
        .context("Failed to query repo")?;

    match rows.next() {
        Some(Ok(id)) => Ok(Some(id)),
        Some(Err(e)) => Err(e).context("Failed to read repo row"),
        None => Ok(None),
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::migrations;
    use rusqlite::Connection;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run(&conn).expect("migrations");
        conn
    }

    #[test]
    fn upsert_creates_row() {
        let conn = db();
        let id = upsert(&conn, "/home/user/myapp").expect("upsert");
        assert!(id > 0);
    }

    #[test]
    fn upsert_is_idempotent_and_returns_same_id() {
        let conn = db();
        let id1 = upsert(&conn, "/home/user/myapp").expect("first upsert");
        let id2 = upsert(&conn, "/home/user/myapp").expect("second upsert");
        assert_eq!(id1, id2);
    }

    #[test]
    fn upsert_derives_name_from_path() {
        let conn = db();
        upsert(&conn, "/home/user/my-cool-repo").expect("upsert");
        let name: String = conn
            .query_row(
                "SELECT name FROM repos WHERE path = ?1",
                rusqlite::params!["/home/user/my-cool-repo"],
                |row| row.get(0),
            )
            .expect("query");
        assert_eq!(name, "my-cool-repo");
    }

    #[test]
    fn find_id_returns_none_for_unknown_path() {
        let conn = db();
        let result = find_id(&conn, "/unknown/path").expect("find");
        assert!(result.is_none());
    }

    #[test]
    fn find_id_returns_some_after_upsert() {
        let conn = db();
        let inserted = upsert(&conn, "/home/user/repo").expect("upsert");
        let found = find_id(&conn, "/home/user/repo")
            .expect("find")
            .expect("should be Some");
        assert_eq!(inserted, found);
    }

    #[test]
    fn load_all_returns_all_repos_sorted_by_last_seen() {
        let conn = db();
        upsert(&conn, "/home/user/repo-a").expect("upsert a");
        upsert(&conn, "/home/user/repo-b").expect("upsert b");
        upsert(&conn, "/home/user/repo-a").expect("re-upsert a to update last_seen");

        let rows = load_all(&conn).expect("load_all");
        assert_eq!(rows.len(), 2);
        // repo-a was touched last, so it appears first
        assert_eq!(rows[0].path, "/home/user/repo-a");
        assert_eq!(rows[0].name, "repo-a");
    }
}
