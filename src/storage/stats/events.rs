//! Append-only event recording.
//!
//! Every `record_*` function is designed to be called with `.ok()` — a stats
//! write failure must never abort the user's actual operation.  Each function
//! is a single `INSERT` statement with no reads, making the overhead <0.1 ms.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::Connection;

/// Record one CLI invocation in `command_runs`.
///
/// Called once per run from `main::run()` after the command completes.
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

/// Record a commit message in `commit_messages`.
///
/// Used both for commits made through `g commit` and for importing git history.
/// The `imported` flag distinguishes backfilled commits from new ones.
// 9 args is a 1:1 mapping to the `commit_messages` columns; bundling them into
// a struct buys nothing because every call site is filling in fresh values.
#[allow(clippy::too_many_arguments)]
pub fn record_commit_message(
    conn: &Connection,
    repo_id: i64,
    commit_hash: &str,
    subject: &str,
    body: Option<&str>,
    author_name: Option<&str>,
    author_email: Option<&str>,
    committed_at: &str,
    imported: bool,
) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO commit_messages
             (repo_id, commit_hash, subject, body, author_name, author_email, committed_at, imported)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            repo_id,
            commit_hash,
            subject,
            body,
            author_name,
            author_email,
            committed_at,
            imported as i32,
        ],
    )
    .context("Failed to record commit message")?;
    Ok(())
}

/// Import commits from git history that are not yet in the database.
///
/// Runs `git log` and inserts any missing commits. Returns the count of newly
/// imported commits.
pub fn import_git_history(conn: &Connection, repo_id: i64, limit: Option<usize>) -> Result<usize> {
    use std::process::Command;

    // Format: hash|author_name|author_email|date|subject|body
    // Using %x00 as field separator and %x01 as record separator
    let limit_arg = limit.map(|n| format!("-n{}", n)).unwrap_or_default();
    let format_arg = "--format=%H%x00%an%x00%ae%x00%aI%x00%s%x00%b%x01".to_string();

    let mut args = vec!["log", "--all", &format_arg];
    if !limit_arg.is_empty() {
        args.push(&limit_arg);
    }

    let output = Command::new("git")
        .args(&args)
        .output()
        .context("Failed to run git log")?;

    if !output.status.success() {
        return Ok(0);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut imported_count = 0;

    for record in stdout.split('\x01') {
        let record = record.trim();
        if record.is_empty() {
            continue;
        }

        let fields: Vec<&str> = record.split('\x00').collect();
        if fields.len() < 5 {
            continue;
        }

        let hash = fields[0];
        let author_name = fields[1];
        let author_email = fields[2];
        let date = fields[3];
        let subject = fields[4];
        let body = fields.get(5).and_then(|b| {
            let b = b.trim();
            if b.is_empty() {
                None
            } else {
                Some(b)
            }
        });

        // Check if commit already exists
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM commit_messages WHERE repo_id = ?1 AND commit_hash = ?2",
                rusqlite::params![repo_id, hash],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if !exists {
            record_commit_message(
                conn,
                repo_id,
                hash,
                subject,
                body,
                Some(author_name),
                Some(author_email),
                date,
                true, // imported = true
            )?;
            imported_count += 1;
        }
    }

    Ok(imported_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{migrations, repos};

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

    #[test]
    fn record_commit_message_inserts_row() {
        let (conn, repo_id) = db();
        record_commit_message(
            &conn,
            repo_id,
            "abc123def456",
            "feat: add new feature",
            Some("This is the body of the commit."),
            Some("Test Author"),
            Some("test@example.com"),
            "2024-01-15T10:30:00Z",
            false,
        )
        .expect("record");
        assert_eq!(count(&conn, "commit_messages"), 1);
    }

    #[test]
    fn record_commit_message_ignores_duplicates() {
        let (conn, repo_id) = db();
        let hash = "abc123def456";

        // First insert should succeed
        record_commit_message(
            &conn,
            repo_id,
            hash,
            "feat: first message",
            None,
            None,
            None,
            "2024-01-15T10:30:00Z",
            false,
        )
        .expect("record");

        // Second insert with same hash should be ignored (INSERT OR IGNORE)
        record_commit_message(
            &conn,
            repo_id,
            hash,
            "feat: second message",
            None,
            None,
            None,
            "2024-01-15T10:30:00Z",
            false,
        )
        .expect("record");

        // Should still only have one row
        assert_eq!(count(&conn, "commit_messages"), 1);

        // And it should be the first message
        let subject: String = conn
            .query_row(
                "SELECT subject FROM commit_messages WHERE commit_hash = ?1",
                [hash],
                |r| r.get(0),
            )
            .expect("query");
        assert_eq!(subject, "feat: first message");
    }
}
