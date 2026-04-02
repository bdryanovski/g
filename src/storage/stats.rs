//! Append-only event recording for usage statistics.
//!
//! All `record_*` functions are designed to be called with `.ok()` — a stats
//! write failure must never abort the user's actual operation.  Each function
//! is a single `INSERT` statement with no reads, making the overhead <0.1 ms.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::Connection;

// ─── Public API ──────────────────────────────────────────────────────────────

/// Record one CLI invocation in `command_runs`.
///
/// Called once per run from `main::run()` after the command completes.
///
/// # Errors
///
/// Returns an error if the SQL fails.
pub fn record_command(
    conn: &Connection,
    command: &str,
    subcommand: Option<&str>,
    repo_id: Option<i64>,
    duration_ms: Option<u64>,
    exit_code: i32,
    error_message: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO command_runs
             (command, subcommand, repo_id, ran_at, duration_ms, exit_code, error_message)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            command,
            subcommand,
            repo_id,
            Utc::now().to_rfc3339(),
            duration_ms.map(|d| d as i64),
            exit_code,
            error_message,
        ],
    )
    .context("Failed to record command run")?;
    Ok(())
}

/// Record a branch-level git event in `branch_events`.
///
/// `event_type` values: `"checkout"` | `"create"` | `"delete"` | `"push"` |
/// `"rebase"`.
///
/// # Errors
///
/// Returns an error if the SQL fails.
pub fn record_branch_event(
    conn: &Connection,
    repo_id: i64,
    branch_name: &str,
    event_type: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO branch_events (repo_id, branch_name, event_type, occurred_at)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![repo_id, branch_name, event_type, Utc::now().to_rfc3339()],
    )
    .context("Failed to record branch event")?;
    Ok(())
}

/// Record a commit made through `g commit` in `commit_records`.
///
/// # Errors
///
/// Returns an error if the SQL fails.
pub fn record_commit(
    conn: &Connection,
    repo_id: i64,
    commit_type: Option<&str>,
    scope: Option<&str>,
    has_body: bool,
    gpg_signed: bool,
) -> Result<()> {
    conn.execute(
        "INSERT INTO commit_records
             (repo_id, commit_type, scope, has_body, gpg_signed, committed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            repo_id,
            commit_type,
            scope,
            has_body as i32,
            gpg_signed as i32,
            Utc::now().to_rfc3339(),
        ],
    )
    .context("Failed to record commit")?;
    Ok(())
}

/// Record a workspace lifecycle event in `workspace_events`.
///
/// `event_type` values: `"create"` | `"switch"` | `"delete"` | `"rename"` |
/// `"init"` | `"clone"`.
///
/// Both `workspace_id` and `repo_id` are optional to handle cases where the
/// workspace row may not yet exist (e.g. recording `"init"` before the row is
/// inserted) or where we are outside a git repo.
///
/// # Errors
///
/// Returns an error if the SQL fails.
pub fn record_workspace_event(
    conn: &Connection,
    workspace_id: Option<i64>,
    repo_id: Option<i64>,
    event_type: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO workspace_events (workspace_id, repo_id, event_type, occurred_at)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![workspace_id, repo_id, event_type, Utc::now().to_rfc3339()],
    )
    .context("Failed to record workspace event")?;
    Ok(())
}

/// Record a stack lifecycle event in `stack_events`.
///
/// `event_type` values: `"create"` | `"add"` | `"push"` | `"pr"` |
/// `"squash"` | `"fold"` | `"sync"` | `"absorb"` | `"delete"` | `"switch"`.
///
/// # Errors
///
/// Returns an error if the SQL fails.
pub fn record_stack_event(
    conn: &Connection,
    stack_id: Option<i64>,
    repo_id: Option<i64>,
    event_type: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO stack_events (stack_id, repo_id, event_type, occurred_at)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![stack_id, repo_id, event_type, Utc::now().to_rfc3339()],
    )
    .context("Failed to record stack event")?;
    Ok(())
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
        let repo_id = repos::upsert(&conn, "/home/user/repo").expect("repo");
        (conn, repo_id)
    }

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap_or(0)
    }

    #[test]
    fn record_command_inserts_row() {
        let (conn, repo_id) = db();
        record_command(&conn, "commit", None, Some(repo_id), Some(42), 0, None).expect("record");
        assert_eq!(count(&conn, "command_runs"), 1);
    }

    #[test]
    fn record_command_with_error() {
        let (conn, _) = db();
        record_command(
            &conn,
            "stack",
            Some("pr"),
            None,
            None,
            1,
            Some("not in a repo"),
        )
        .expect("record");
        let msg: String = conn
            .query_row(
                "SELECT error_message FROM command_runs WHERE exit_code = 1",
                [],
                |r| r.get(0),
            )
            .expect("query");
        assert_eq!(msg, "not in a repo");
    }

    #[test]
    fn record_branch_event_inserts_row() {
        let (conn, repo_id) = db();
        record_branch_event(&conn, repo_id, "feat/auth", "create").expect("record");
        assert_eq!(count(&conn, "branch_events"), 1);
    }

    #[test]
    fn record_commit_inserts_row() {
        let (conn, repo_id) = db();
        record_commit(&conn, repo_id, Some("feat"), Some("auth"), true, false).expect("record");
        assert_eq!(count(&conn, "commit_records"), 1);
    }

    #[test]
    fn record_workspace_event_inserts_row() {
        let (conn, repo_id) = db();
        record_workspace_event(&conn, None, Some(repo_id), "create").expect("record");
        assert_eq!(count(&conn, "workspace_events"), 1);
    }

    #[test]
    fn record_stack_event_inserts_row() {
        let (conn, repo_id) = db();
        record_stack_event(&conn, None, Some(repo_id), "create").expect("record");
        assert_eq!(count(&conn, "stack_events"), 1);
    }
}
