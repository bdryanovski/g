//! Append-only event recording for usage statistics, plus read-side queries.
//!
//! All `record_*` functions are designed to be called with `.ok()` — a stats
//! write failure must never abort the user's actual operation.  Each function
//! is a single `INSERT` statement with no reads, making the overhead <0.1 ms.
//!
//! The `query_*` / `top_*` / `streak_info` / `activity_by_hour` functions are
//! read-only helpers used by the `g stats` command to aggregate data.

use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc};
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

// ─── Read-side query helpers ─────────────────────────────────────────────────

/// Aggregate totals pulled from all stats tables.
#[derive(Debug, Default)]
pub struct OverallStats {
    /// Total rows in `command_runs`.
    pub total_commands: i64,
    /// Total rows in `commit_records` (commits via `g commit`).
    pub total_commits_recorded: i64,
    /// Total distinct repos tracked.
    pub total_repos: i64,
    /// Commands that exited with a non-zero status.
    pub total_errors: i64,
    /// Distinct calendar days on which at least one command ran.
    pub active_days: i64,
    /// Mean wall-clock duration across all timed command runs (ms).
    pub avg_duration_ms: f64,
}

/// Return aggregate totals from all stats tables.
///
/// Every query falls back to `0` on error so a missing or empty table never
/// prevents the rest of the report from rendering.
///
/// # Errors
///
/// Currently infallible; always returns `Ok(OverallStats)`.
pub fn query_overall(conn: &Connection) -> Result<OverallStats> {
    let total_commands: i64 = conn
        .query_row("SELECT COUNT(*) FROM command_runs", [], |r| r.get(0))
        .unwrap_or(0);

    let total_commits_recorded: i64 = conn
        .query_row("SELECT COUNT(*) FROM commit_records", [], |r| r.get(0))
        .unwrap_or(0);

    let total_repos: i64 = conn
        .query_row("SELECT COUNT(*) FROM repos", [], |r| r.get(0))
        .unwrap_or(0);

    let total_errors: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM command_runs WHERE exit_code != 0",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let active_days: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT substr(ran_at, 1, 10)) FROM command_runs",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let avg_duration_ms: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(duration_ms), 0.0) \
             FROM command_runs WHERE duration_ms IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    Ok(OverallStats {
        total_commands,
        total_commits_recorded,
        total_repos,
        total_errors,
        active_days,
        avg_duration_ms,
    })
}

/// Return command-run counts grouped by calendar day for the last `days` days.
///
/// Result is sorted by date ascending; missing days are absent (the caller
/// fills them in with zero when building the heatmap).
///
/// # Errors
///
/// Returns an error if the SQL statement cannot be prepared or executed.
#[allow(dead_code)]
pub fn command_run_counts_per_day(conn: &Connection, days: u32) -> Result<Vec<(String, i64)>> {
    let cutoff = (Utc::now() - chrono::Duration::days(i64::from(days)))
        .format("%Y-%m-%d")
        .to_string();

    let mut stmt = conn.prepare(
        "SELECT substr(ran_at, 1, 10) AS day, COUNT(*) AS cnt
         FROM command_runs
         WHERE substr(ran_at, 1, 10) >= ?1
         GROUP BY day
         ORDER BY day",
    )?;

    let rows: Vec<(String, i64)> = stmt
        .query_map([cutoff], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

/// Return the top `limit` most-used commands.
///
/// Combines command + subcommand into one label (e.g. `"workspace create"`).
///
/// # Errors
///
/// Returns an error if the SQL statement cannot be prepared or executed.
pub fn top_commands(conn: &Connection, limit: usize) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT
           CASE WHEN subcommand IS NOT NULL
                THEN command || ' ' || subcommand
                ELSE command END AS full_cmd,
           COUNT(*) AS cnt
         FROM command_runs
         GROUP BY full_cmd
         ORDER BY cnt DESC
         LIMIT ?1",
    )?;

    let rows: Vec<(String, i64)> = stmt
        .query_map([limit as i64], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

/// Return counts of each conventional-commit type recorded in `commit_records`.
///
/// NULL commit types are grouped under `"other"`.
///
/// # Errors
///
/// Returns an error if the SQL statement cannot be prepared or executed.
pub fn commit_type_counts(conn: &Connection) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(commit_type, 'other') AS typ, COUNT(*) AS cnt
         FROM commit_records
         GROUP BY typ
         ORDER BY cnt DESC",
    )?;

    let rows: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

/// Return the top `limit` repositories by total command-run count.
///
/// # Errors
///
/// Returns an error if the SQL statement cannot be prepared or executed.
pub fn top_repos_by_activity(conn: &Connection, limit: usize) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT r.name, COUNT(cr.id) AS cnt
         FROM repos r
         LEFT JOIN command_runs cr ON cr.repo_id = r.id
         GROUP BY r.id, r.name
         ORDER BY cnt DESC
         LIMIT ?1",
    )?;

    let rows: Vec<(String, i64)> = stmt
        .query_map([limit as i64], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

/// Return command-run counts grouped by hour of day (0–23).
///
/// Hours with no activity are absent from the result.
///
/// # Errors
///
/// Returns an error if the SQL statement cannot be prepared or executed.
pub fn activity_by_hour(conn: &Connection) -> Result<Vec<(u32, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT CAST(substr(ran_at, 12, 2) AS INTEGER) AS hour, COUNT(*) AS cnt
         FROM command_runs
         WHERE length(ran_at) >= 13
         GROUP BY hour
         ORDER BY hour",
    )?;

    let rows: Vec<(u32, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

/// Return `(current_streak, longest_streak)` in active days.
///
/// *Current streak*: the number of consecutive days ending today or yesterday
/// on which at least one command ran.
/// *Longest streak*: the all-time longest consecutive-day run in the database.
///
/// # Errors
///
/// Returns an error if the SQL statement cannot be prepared or executed.
pub fn streak_info(conn: &Connection) -> Result<(u32, u32)> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT substr(ran_at, 1, 10) AS day \
         FROM command_runs ORDER BY day DESC",
    )?;

    let days: Vec<NaiveDate> = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .filter_map(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
        .collect();

    if days.is_empty() {
        return Ok((0, 0));
    }

    let today = Utc::now().date_naive();

    // Current streak: walk backwards from today/yesterday.
    let mut current_streak = 0u32;
    let first = days[0];
    if (today - first).num_days() <= 1 {
        let mut expected = first;
        for &day in &days {
            if day == expected {
                current_streak += 1;
                expected = match expected.pred_opt() {
                    Some(d) => d,
                    None => break,
                };
            } else {
                break;
            }
        }
    }

    // Longest streak: scan all days in ascending order.
    let mut longest_streak = 0u32;
    let mut run = 0u32;
    let mut prev: Option<NaiveDate> = None;
    for &day in days.iter().rev() {
        if let Some(p) = prev {
            if (day - p).num_days() == 1 {
                run += 1;
            } else {
                run = 1;
            }
        } else {
            run = 1;
        }
        longest_streak = longest_streak.max(run);
        prev = Some(day);
    }

    Ok((current_streak, longest_streak))
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
