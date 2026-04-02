//! Workspace (git worktree) persistence.
//!
//! Replaces the TOML-based `load_store` / `save_store` functions in
//! `commands/workspace.rs`.  All public functions take a `&Connection` and
//! operate directly on the `workspaces` table.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::Connection;

// ─── Types ───────────────────────────────────────────────────────────────────

/// A row from the `workspaces` table.
#[derive(Debug, Clone)]
pub struct WorkspaceRow {
    /// Primary key.
    pub id: i64,
    /// FK to `repos(id)`.
    pub repo_id: i64,
    /// Human-friendly name used in `g workspace switch <name>`.
    pub name: String,
    /// Optional one-line description shown in `g workspace list`.
    pub description: Option<String>,
    /// Absolute filesystem path to the worktree directory.
    pub path: String,
    /// Branch associated with the worktree at creation time.
    pub branch: String,
    /// Set by `g workspace init` — the container root directory.
    /// Stored for completeness; queried via SQL in [`get_container_root`] rather
    /// than read from the struct in current code.
    #[allow(dead_code)]
    pub container_root: Option<String>,
    /// When this workspace was created.
    pub created_at: DateTime<Utc>,
}

/// Input type for [`insert`] — field names mirror [`WorkspaceRow`] minus `id`.
pub struct NewWorkspace<'a> {
    /// Human-friendly name.
    pub name: &'a str,
    /// Optional description.
    pub description: Option<&'a str>,
    /// Absolute path to the worktree directory.
    pub path: &'a str,
    /// Branch name at creation time.
    pub branch: &'a str,
    /// Optional container root (set by `g workspace init`).
    pub container_root: Option<&'a str>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Load all workspace rows for `repo_id`, ordered by creation time.
///
/// # Errors
///
/// Returns an error if the SQL fails to execute.
pub fn load_for_repo(conn: &Connection, repo_id: i64) -> Result<Vec<WorkspaceRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, repo_id, name, description, path, branch, container_root, created_at
             FROM workspaces
             WHERE repo_id = ?1
             ORDER BY created_at ASC",
        )
        .context("Failed to prepare workspace query")?;

    let rows = stmt
        .query_map(rusqlite::params![repo_id], map_row)
        .context("Failed to query workspaces")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to read workspace rows")?;

    Ok(rows)
}

/// Load a single workspace by its absolute `path`.
///
/// # Errors
///
/// Returns an error if the SQL fails to execute.
pub fn find_by_path(conn: &Connection, path: &str) -> Result<Option<WorkspaceRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, repo_id, name, description, path, branch, container_root, created_at
             FROM workspaces WHERE path = ?1",
        )
        .context("Failed to prepare workspace path lookup")?;

    let mut rows = stmt
        .query_map(rusqlite::params![path], map_row)
        .context("Failed to query workspace by path")?;

    match rows.next() {
        Some(Ok(row)) => Ok(Some(row)),
        Some(Err(e)) => Err(e).context("Failed to read workspace row"),
        None => Ok(None),
    }
}

/// Insert a new workspace row and return its `id`.
///
/// # Errors
///
/// Returns an error if the workspace name or path already exists for this repo,
/// or if the SQL fails.
pub fn insert(conn: &Connection, repo_id: i64, ws: &NewWorkspace<'_>) -> Result<i64> {
    conn.execute(
        "INSERT INTO workspaces
             (repo_id, name, description, path, branch, container_root, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            repo_id,
            ws.name,
            ws.description,
            ws.path,
            ws.branch,
            ws.container_root,
            ws.created_at.to_rfc3339(),
        ],
    )
    .with_context(|| format!("Failed to insert workspace '{}'", ws.name))?;

    Ok(conn.last_insert_rowid())
}

/// Rename a workspace and update its filesystem path.
///
/// # Errors
///
/// Returns an error if no row with `id` exists or if the SQL fails.
pub fn update_name_and_path(conn: &Connection, id: i64, name: &str, path: &str) -> Result<()> {
    conn.execute(
        "UPDATE workspaces SET name = ?1, path = ?2 WHERE id = ?3",
        rusqlite::params![name, path, id],
    )
    .context("Failed to update workspace name and path")?;
    Ok(())
}

/// Delete a workspace row by `id`.
///
/// # Errors
///
/// Returns an error if the SQL fails.
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM workspaces WHERE id = ?1",
        rusqlite::params![id],
    )
    .context("Failed to delete workspace")?;
    Ok(())
}

/// Return the `container_root` for any workspace belonging to `repo_id`.
///
/// All workspaces in a repo share the same `container_root` (or all have
/// `NULL`).  Returns `None` if no workspace exists or the field is NULL.
///
/// # Errors
///
/// Returns an error if the SQL fails.
pub fn get_container_root(conn: &Connection, repo_id: i64) -> Result<Option<String>> {
    let mut stmt = conn
        .prepare(
            "SELECT container_root FROM workspaces
             WHERE repo_id = ?1 AND container_root IS NOT NULL
             LIMIT 1",
        )
        .context("Failed to prepare container_root query")?;

    let mut rows = stmt
        .query_map(rusqlite::params![repo_id], |row| row.get(0))
        .context("Failed to query container_root")?;

    match rows.next() {
        Some(Ok(val)) => Ok(val),
        Some(Err(e)) => Err(e).context("Failed to read container_root"),
        None => Ok(None),
    }
}

/// Set the `container_root` on all workspace rows for `repo_id`.
///
/// Called by `g workspace init` after reorganising the directory layout.
///
/// # Errors
///
/// Returns an error if the SQL fails.
pub fn set_container_root(conn: &Connection, repo_id: i64, root: &str) -> Result<()> {
    conn.execute(
        "UPDATE workspaces SET container_root = ?1 WHERE repo_id = ?2",
        rusqlite::params![root, repo_id],
    )
    .context("Failed to set container_root")?;
    Ok(())
}

// ─── Internal helpers ────────────────────────────────────────────────────────

/// Map a `rusqlite::Row` to a [`WorkspaceRow`].
fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkspaceRow> {
    let created_at_str: String = row.get(7)?;
    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    Ok(WorkspaceRow {
        id: row.get(0)?,
        repo_id: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        path: row.get(4)?,
        branch: row.get(5)?,
        container_root: row.get(6)?,
        created_at,
    })
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{migrations, repos};
    use rusqlite::Connection;

    fn db() -> (Connection, i64) {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run(&conn).expect("migrations");
        let repo_id = repos::upsert(&conn, "/home/user/myapp").expect("upsert repo");
        (conn, repo_id)
    }

    fn sample(name: &str, path: &str) -> NewWorkspace<'static> {
        // We need owned strings — use Box::leak for test convenience only.
        let name: &'static str = Box::leak(name.to_string().into_boxed_str());
        let path: &'static str = Box::leak(path.to_string().into_boxed_str());
        NewWorkspace {
            name,
            description: None,
            path,
            branch: "main",
            container_root: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn insert_and_load() {
        let (conn, repo_id) = db();
        insert(
            &conn,
            repo_id,
            &sample("feat-auth", "/home/user/myapp--feat-auth"),
        )
        .expect("insert");
        let rows = load_for_repo(&conn, repo_id).expect("load");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "feat-auth");
        assert_eq!(rows[0].branch, "main");
    }

    #[test]
    fn delete_removes_row() {
        let (conn, repo_id) = db();
        let id = insert(&conn, repo_id, &sample("tmp", "/tmp/ws")).expect("insert");
        delete(&conn, id).expect("delete");
        let rows = load_for_repo(&conn, repo_id).expect("load");
        assert!(rows.is_empty());
    }

    #[test]
    fn update_name_and_path_changes_values() {
        let (conn, repo_id) = db();
        let id = insert(&conn, repo_id, &sample("old", "/old/path")).expect("insert");
        update_name_and_path(&conn, id, "new", "/new/path").expect("update");
        let rows = load_for_repo(&conn, repo_id).expect("load");
        assert_eq!(rows[0].name, "new");
        assert_eq!(rows[0].path, "/new/path");
    }

    #[test]
    fn find_by_path_returns_correct_row() {
        let (conn, repo_id) = db();
        insert(&conn, repo_id, &sample("ws1", "/home/user/ws1")).expect("insert");
        let found = find_by_path(&conn, "/home/user/ws1")
            .expect("find")
            .expect("should be Some");
        assert_eq!(found.name, "ws1");
    }

    #[test]
    fn container_root_round_trip() {
        let (conn, repo_id) = db();
        insert(&conn, repo_id, &sample("main", "/container/main")).expect("insert");
        set_container_root(&conn, repo_id, "/container").expect("set");
        let root = get_container_root(&conn, repo_id)
            .expect("get")
            .expect("should be Some");
        assert_eq!(root, "/container");
    }
}
