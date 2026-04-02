//! Stacked-PR metadata persistence.
//!
//! Replaces the TOML-based `load_store` / `save_store` functions in
//! `commands/stack.rs`.  Stacks and their ordered branch lists are stored in
//! the `stacks` and `stack_branches` tables respectively.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::Connection;

// ─── Types ───────────────────────────────────────────────────────────────────

/// A row from the `stacks` table, with its branches eagerly loaded.
#[derive(Debug, Clone)]
pub struct StackRow {
    /// Primary key.
    pub id: i64,
    /// FK to `repos(id)`.
    pub repo_id: i64,
    /// Human-readable stack name.
    pub name: String,
    /// The branch at the bottom of the stack — the eventual merge target.
    pub root_branch: String,
    /// When the stack was first created.
    pub created_at: DateTime<Utc>,
    /// When the stack was last modified.
    pub updated_at: DateTime<Utc>,
    /// Ordered branches from bottom (index 0) to top.
    pub branches: Vec<StackBranchRow>,
}

/// A row from the `stack_branches` table.
///
/// The database `id` and `stack_id` columns are deliberately omitted — branch
/// identity within the application is always `(stack_id, name)`, and the
/// owning `StackRow.id` provides `stack_id` from context.
#[derive(Debug, Clone)]
pub struct StackBranchRow {
    /// 0-based position within the stack (bottom = 0).
    pub position: i32,
    /// Short branch name.
    pub name: String,
    /// GitHub PR number, if a PR has been created.
    pub pr_number: Option<u64>,
    /// GitHub PR web URL, if a PR has been created.
    pub pr_url: Option<String>,
    /// Optional one-line description.
    pub description: Option<String>,
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Load all stacks for `repo_id` with their branches, ordered by creation time.
///
/// # Errors
///
/// Returns an error if any SQL fails.
pub fn load_all(conn: &Connection, repo_id: i64) -> Result<Vec<StackRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, repo_id, name, root_branch, created_at, updated_at
             FROM stacks WHERE repo_id = ?1 ORDER BY created_at ASC",
        )
        .context("Failed to prepare stacks query")?;

    let mut stacks: Vec<StackRow> = stmt
        .query_map(rusqlite::params![repo_id], map_stack_row)
        .context("Failed to query stacks")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to read stack rows")?;

    for stack in &mut stacks {
        stack.branches = load_branches(conn, stack.id)?;
    }

    Ok(stacks)
}

/// Load a single stack by name for `repo_id`, with its branches.
///
/// Returns `None` if no stack with that name exists.
///
/// # Errors
///
/// Returns an error if any SQL fails.
pub fn load_by_name(conn: &Connection, repo_id: i64, name: &str) -> Result<Option<StackRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, repo_id, name, root_branch, created_at, updated_at
             FROM stacks WHERE repo_id = ?1 AND name = ?2",
        )
        .context("Failed to prepare stack lookup")?;

    let mut rows = stmt
        .query_map(rusqlite::params![repo_id, name], map_stack_row)
        .context("Failed to query stack by name")?;

    match rows.next() {
        Some(Ok(mut stack)) => {
            stack.branches = load_branches(conn, stack.id)?;
            Ok(Some(stack))
        }
        Some(Err(e)) => Err(e).context("Failed to read stack row"),
        None => Ok(None),
    }
}

/// Insert a new stack and return its `id`.
///
/// The stack starts with no branches; use [`set_branches`] to populate them.
///
/// # Errors
///
/// Returns an error if a stack with this name already exists for the repo, or
/// if the SQL fails.
pub fn insert(conn: &Connection, repo_id: i64, name: &str, root_branch: &str) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO stacks (repo_id, name, root_branch, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        rusqlite::params![repo_id, name, root_branch, now],
    )
    .with_context(|| format!("Failed to insert stack '{name}'"))?;

    Ok(conn.last_insert_rowid())
}

/// Update the `updated_at` timestamp on a stack to now.
///
/// Call this after any mutation to the stack's branches.
///
/// # Errors
///
/// Returns an error if the SQL fails.
pub fn touch(conn: &Connection, id: i64) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE stacks SET updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id],
    )
    .context("Failed to update stack timestamp")?;
    Ok(())
}

/// Delete a stack and all its branches (CASCADE handles `stack_branches`).
///
/// # Errors
///
/// Returns an error if the SQL fails.
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM stacks WHERE id = ?1", rusqlite::params![id])
        .context("Failed to delete stack")?;
    Ok(())
}

/// Replace all branches for `stack_id` with the given ordered list.
///
/// This runs inside a transaction: it deletes all existing branch rows then
/// bulk-inserts the new ones.  The caller is responsible for setting correct
/// `position` values (0-based, bottom → top).
///
/// # Errors
///
/// Returns an error if any SQL fails.
pub fn set_branches(conn: &Connection, stack_id: i64, branches: &[StackBranchRow]) -> Result<()> {
    conn.execute(
        "DELETE FROM stack_branches WHERE stack_id = ?1",
        rusqlite::params![stack_id],
    )
    .context("Failed to clear stack branches")?;

    for branch in branches {
        conn.execute(
            "INSERT INTO stack_branches
                 (stack_id, position, name, pr_number, pr_url, description)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                stack_id,
                branch.position,
                branch.name,
                branch.pr_number.map(|n| n as i64),
                branch.pr_url,
                branch.description,
            ],
        )
        .with_context(|| format!("Failed to insert branch '{}'", branch.name))?;
    }

    touch(conn, stack_id)?;
    Ok(())
}

/// Update the PR number and URL for a specific branch within a stack.
///
/// # Errors
///
/// Returns an error if the SQL fails.
pub fn update_branch_pr(
    conn: &Connection,
    stack_id: i64,
    branch_name: &str,
    pr_number: u64,
    pr_url: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE stack_branches SET pr_number = ?1, pr_url = ?2
         WHERE stack_id = ?3 AND name = ?4",
        rusqlite::params![pr_number as i64, pr_url, stack_id, branch_name],
    )
    .with_context(|| format!("Failed to update PR for branch '{branch_name}'"))?;

    touch(conn, stack_id)?;
    Ok(())
}

// ─── Internal helpers ────────────────────────────────────────────────────────

fn load_branches(conn: &Connection, stack_id: i64) -> Result<Vec<StackBranchRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT position, name, pr_number, pr_url, description
             FROM stack_branches WHERE stack_id = ?1 ORDER BY position ASC",
        )
        .context("Failed to prepare branch query")?;

    let branches = stmt
        .query_map(rusqlite::params![stack_id], map_branch_row)
        .context("Failed to query stack branches")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to read branch rows")?;

    Ok(branches)
}

fn map_stack_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StackRow> {
    let created_at = parse_dt(row.get::<_, String>(4)?);
    let updated_at = parse_dt(row.get::<_, String>(5)?);

    Ok(StackRow {
        id: row.get(0)?,
        repo_id: row.get(1)?,
        name: row.get(2)?,
        root_branch: row.get(3)?,
        created_at,
        updated_at,
        branches: vec![],
    })
}

fn map_branch_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StackBranchRow> {
    let pr_number: Option<i64> = row.get(2)?;
    Ok(StackBranchRow {
        position: row.get(0)?,
        name: row.get(1)?,
        pr_number: pr_number.map(|n| n as u64),
        pr_url: row.get(3)?,
        description: row.get(4)?,
    })
}

fn parse_dt(s: String) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
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
        let repo_id = repos::upsert(&conn, "/home/user/repo").expect("upsert repo");
        (conn, repo_id)
    }

    fn branch(pos: i32, name: &str) -> StackBranchRow {
        StackBranchRow {
            position: pos,
            name: name.to_string(),
            pr_number: None,
            pr_url: None,
            description: None,
        }
    }

    #[test]
    fn insert_and_load_all() {
        let (conn, repo_id) = db();
        insert(&conn, repo_id, "my-stack", "main").expect("insert");
        let stacks = load_all(&conn, repo_id).expect("load");
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].name, "my-stack");
        assert_eq!(stacks[0].root_branch, "main");
    }

    #[test]
    fn set_branches_and_load() {
        let (conn, repo_id) = db();
        let id = insert(&conn, repo_id, "stack", "main").expect("insert");
        let branches = vec![branch(0, "feat-a"), branch(1, "feat-b")];
        set_branches(&conn, id, &branches).expect("set");

        let loaded = load_by_name(&conn, repo_id, "stack")
            .expect("load")
            .expect("Some");
        assert_eq!(loaded.branches.len(), 2);
        assert_eq!(loaded.branches[0].name, "feat-a");
        assert_eq!(loaded.branches[1].name, "feat-b");
    }

    #[test]
    fn set_branches_replaces_existing() {
        let (conn, repo_id) = db();
        let id = insert(&conn, repo_id, "stack", "main").expect("insert");
        set_branches(&conn, id, &[branch(0, "old-a"), branch(1, "old-b")]).expect("first set");
        set_branches(&conn, id, &[branch(0, "new-only")]).expect("second set");

        let loaded = load_by_name(&conn, repo_id, "stack")
            .expect("load")
            .expect("Some");
        assert_eq!(loaded.branches.len(), 1);
        assert_eq!(loaded.branches[0].name, "new-only");
    }

    #[test]
    fn delete_removes_stack_and_branches() {
        let (conn, repo_id) = db();
        let id = insert(&conn, repo_id, "stack", "main").expect("insert");
        set_branches(&conn, id, &[branch(0, "feat")]).expect("set");
        delete(&conn, id).expect("delete");

        assert!(load_all(&conn, repo_id).expect("load").is_empty());
        let branch_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM stack_branches WHERE stack_id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(branch_count, 0);
    }

    #[test]
    fn update_branch_pr_sets_fields() {
        let (conn, repo_id) = db();
        let id = insert(&conn, repo_id, "stack", "main").expect("insert");
        set_branches(&conn, id, &[branch(0, "feat")]).expect("set");
        update_branch_pr(&conn, id, "feat", 42, "https://github.com/pr/42").expect("update_pr");

        let loaded = load_by_name(&conn, repo_id, "stack")
            .expect("load")
            .expect("Some");
        assert_eq!(loaded.branches[0].pr_number, Some(42));
        assert_eq!(
            loaded.branches[0].pr_url.as_deref(),
            Some("https://github.com/pr/42")
        );
    }
}
