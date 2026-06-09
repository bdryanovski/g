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

/// Record a commit message in `commit_messages`.
///
/// Used both for commits made through `g commit` and for importing git history.
/// The `imported` flag distinguishes backfilled commits from new ones.
///
/// # Errors
///
/// Returns an error if the SQL fails.
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
/// Runs `git log` and inserts any missing commits. Returns the count of
/// newly imported commits.
///
/// # Errors
///
/// Returns an error if git commands fail or database operations fail.
pub fn import_git_history(conn: &Connection, repo_id: i64, limit: Option<usize>) -> Result<usize> {
    use std::process::Command;

    // Format: hash|author_name|author_email|date|subject|body
    // Using %x00 as field separator and %x01 as record separator
    let limit_arg = limit.map(|n| format!("-n{}", n)).unwrap_or_default();
    let format_arg = format!("--format=%H%x00%an%x00%ae%x00%aI%x00%s%x00%b%x01");

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

// ─── Commit message queries ──────────────────────────────────────────────────

/// Result of a commit message search.
#[derive(Debug, Clone)]
pub struct CommitSearchResult {
    pub commit_hash: String,
    pub subject: String,
    pub body: Option<String>,
    pub author_name: Option<String>,
    pub committed_at: String,
    pub repo_name: String,
}

/// Search commit messages using full-text search (fuzzy matching).
///
/// Uses SQLite FTS5 for fast text search across subject and body.
///
/// # Errors
///
/// Returns an error if the SQL statement cannot be prepared or executed.
pub fn search_commits(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<CommitSearchResult>> {
    // FTS5 query: match tokens in subject or body
    let fts_query = query
        .split_whitespace()
        .map(|w| format!("\"{}\"*", w.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ");

    let mut stmt = conn.prepare(
        "SELECT cm.commit_hash, cm.subject, cm.body, cm.author_name, cm.committed_at, r.name
         FROM commit_messages cm
         JOIN commit_messages_fts fts ON cm.id = fts.rowid
         JOIN repos r ON cm.repo_id = r.id
         WHERE commit_messages_fts MATCH ?1
         ORDER BY cm.committed_at DESC
         LIMIT ?2",
    )?;

    let rows: Vec<CommitSearchResult> = stmt
        .query_map(rusqlite::params![fts_query, limit as i64], |r| {
            Ok(CommitSearchResult {
                commit_hash: r.get(0)?,
                subject: r.get(1)?,
                body: r.get(2)?,
                author_name: r.get(3)?,
                committed_at: r.get(4)?,
                repo_name: r.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

/// Find duplicate commit messages (same subject appearing multiple times).
///
/// Returns subjects that appear more than once, with their count.
///
/// # Errors
///
/// Returns an error if the SQL statement cannot be prepared or executed.
pub fn find_duplicate_commits(
    conn: &Connection,
    repo_id: Option<i64>,
    limit: usize,
) -> Result<Vec<(String, i64)>> {
    let (sql, params): (&str, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(rid) = repo_id {
        (
            "SELECT subject, COUNT(*) AS cnt
             FROM commit_messages
             WHERE repo_id = ?1
             GROUP BY subject
             HAVING cnt > 1
             ORDER BY cnt DESC
             LIMIT ?2",
            vec![Box::new(rid), Box::new(limit as i64)],
        )
    } else {
        (
            "SELECT subject, COUNT(*) AS cnt
             FROM commit_messages
             GROUP BY subject
             HAVING cnt > 1
             ORDER BY cnt DESC
             LIMIT ?1",
            vec![Box::new(limit as i64)],
        )
    };

    let mut stmt = conn.prepare(sql)?;
    let rows: Vec<(String, i64)> = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

/// Statistics about commit message lengths over time.
#[derive(Debug, Default)]
pub struct CommitLengthStats {
    /// Average subject line length (characters).
    pub avg_subject_length: f64,
    /// Average body length (characters, excluding commits without body).
    pub avg_body_length: f64,
    /// Percentage of commits that have a body.
    pub body_percentage: f64,
    /// Total commits analyzed.
    pub total_commits: i64,
    /// Commits with subject > 72 chars (conventional limit).
    pub long_subjects: i64,
}

/// Get commit message length statistics.
///
/// # Errors
///
/// Returns an error if the SQL statement cannot be prepared or executed.
pub fn commit_length_stats(conn: &Connection, repo_id: Option<i64>) -> Result<CommitLengthStats> {
    let where_clause = repo_id
        .map(|_| "WHERE repo_id = ?1")
        .unwrap_or("");

    let total_commits: i64 = if let Some(rid) = repo_id {
        conn.query_row(
            &format!("SELECT COUNT(*) FROM commit_messages {}", where_clause),
            rusqlite::params![rid],
            |r| r.get(0),
        )
        .unwrap_or(0)
    } else {
        conn.query_row("SELECT COUNT(*) FROM commit_messages", [], |r| r.get(0))
            .unwrap_or(0)
    };

    if total_commits == 0 {
        return Ok(CommitLengthStats::default());
    }

    let avg_subject_length: f64 = if let Some(rid) = repo_id {
        conn.query_row(
            &format!(
                "SELECT COALESCE(AVG(LENGTH(subject)), 0) FROM commit_messages {}",
                where_clause
            ),
            rusqlite::params![rid],
            |r| r.get(0),
        )
        .unwrap_or(0.0)
    } else {
        conn.query_row(
            "SELECT COALESCE(AVG(LENGTH(subject)), 0) FROM commit_messages",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0)
    };

    let avg_body_length: f64 = if let Some(rid) = repo_id {
        conn.query_row(
            &format!(
                "SELECT COALESCE(AVG(LENGTH(body)), 0) FROM commit_messages {} AND body IS NOT NULL AND body != ''",
                if repo_id.is_some() { "WHERE repo_id = ?1" } else { "" }
            ),
            rusqlite::params![rid],
            |r| r.get(0),
        )
        .unwrap_or(0.0)
    } else {
        conn.query_row(
            "SELECT COALESCE(AVG(LENGTH(body)), 0) FROM commit_messages WHERE body IS NOT NULL AND body != ''",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0)
    };

    let commits_with_body: i64 = if let Some(rid) = repo_id {
        conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM commit_messages {} AND body IS NOT NULL AND body != ''",
                if repo_id.is_some() { "WHERE repo_id = ?1" } else { "" }
            ),
            rusqlite::params![rid],
            |r| r.get(0),
        )
        .unwrap_or(0)
    } else {
        conn.query_row(
            "SELECT COUNT(*) FROM commit_messages WHERE body IS NOT NULL AND body != ''",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    };

    let long_subjects: i64 = if let Some(rid) = repo_id {
        conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM commit_messages {} AND LENGTH(subject) > 72",
                if repo_id.is_some() { "WHERE repo_id = ?1" } else { "" }
            ),
            rusqlite::params![rid],
            |r| r.get(0),
        )
        .unwrap_or(0)
    } else {
        conn.query_row(
            "SELECT COUNT(*) FROM commit_messages WHERE LENGTH(subject) > 72",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    };

    let body_percentage = (commits_with_body as f64 / total_commits as f64) * 100.0;

    Ok(CommitLengthStats {
        avg_subject_length,
        avg_body_length,
        body_percentage,
        total_commits,
        long_subjects,
    })
}

/// Commit message length over time (grouped by month).
#[derive(Debug)]
pub struct MonthlyLengthStats {
    pub month: String, // YYYY-MM format
    pub avg_subject_length: f64,
    pub commit_count: i64,
}

/// Get commit message length trends over time (monthly).
///
/// # Errors
///
/// Returns an error if the SQL statement cannot be prepared or executed.
pub fn commit_length_over_time(
    conn: &Connection,
    repo_id: Option<i64>,
    months: usize,
) -> Result<Vec<MonthlyLengthStats>> {
    let cutoff = (Utc::now() - chrono::Duration::days((months * 30) as i64))
        .format("%Y-%m")
        .to_string();

    let sql = if repo_id.is_some() {
        "SELECT substr(committed_at, 1, 7) AS month,
                AVG(LENGTH(subject)) AS avg_len,
                COUNT(*) AS cnt
         FROM commit_messages
         WHERE repo_id = ?1 AND substr(committed_at, 1, 7) >= ?2
         GROUP BY month
         ORDER BY month"
    } else {
        "SELECT substr(committed_at, 1, 7) AS month,
                AVG(LENGTH(subject)) AS avg_len,
                COUNT(*) AS cnt
         FROM commit_messages
         WHERE substr(committed_at, 1, 7) >= ?1
         GROUP BY month
         ORDER BY month"
    };

    let mut stmt = conn.prepare(sql)?;
    let rows: Vec<MonthlyLengthStats> = if let Some(rid) = repo_id {
        stmt.query_map(rusqlite::params![rid, cutoff], |r| {
            Ok(MonthlyLengthStats {
                month: r.get(0)?,
                avg_subject_length: r.get(1)?,
                commit_count: r.get(2)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect()
    } else {
        stmt.query_map(rusqlite::params![cutoff], |r| {
            Ok(MonthlyLengthStats {
                month: r.get(0)?,
                avg_subject_length: r.get(1)?,
                commit_count: r.get(2)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect()
    };

    Ok(rows)
}

/// Top authors by commit count.
///
/// # Errors
///
/// Returns an error if the SQL statement cannot be prepared or executed.
pub fn top_authors(
    conn: &Connection,
    repo_id: Option<i64>,
    limit: usize,
) -> Result<Vec<(String, i64)>> {
    let sql = if repo_id.is_some() {
        "SELECT COALESCE(author_name, 'Unknown') AS author, COUNT(*) AS cnt
         FROM commit_messages
         WHERE repo_id = ?1
         GROUP BY author
         ORDER BY cnt DESC
         LIMIT ?2"
    } else {
        "SELECT COALESCE(author_name, 'Unknown') AS author, COUNT(*) AS cnt
         FROM commit_messages
         GROUP BY author
         ORDER BY cnt DESC
         LIMIT ?1"
    };

    let mut stmt = conn.prepare(sql)?;
    let rows: Vec<(String, i64)> = if let Some(rid) = repo_id {
        stmt.query_map(rusqlite::params![rid, limit as i64], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect()
    } else {
        stmt.query_map(rusqlite::params![limit as i64], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect()
    };

    Ok(rows)
}

/// Get total commit messages count.
///
/// # Errors
///
/// Returns an error if the query fails.
pub fn total_commit_messages(conn: &Connection, repo_id: Option<i64>) -> Result<i64> {
    let count: i64 = if let Some(rid) = repo_id {
        conn.query_row(
            "SELECT COUNT(*) FROM commit_messages WHERE repo_id = ?1",
            rusqlite::params![rid],
            |r| r.get(0),
        )
        .unwrap_or(0)
    } else {
        conn.query_row("SELECT COUNT(*) FROM commit_messages", [], |r| r.get(0))
            .unwrap_or(0)
    };
    Ok(count)
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

    #[test]
    fn find_duplicate_commits_works() {
        let (conn, repo_id) = db();

        // Insert commits with duplicate subjects
        for i in 0..3 {
            record_commit_message(
                &conn,
                repo_id,
                &format!("hash{}", i),
                "fix: typo",
                None,
                None,
                None,
                "2024-01-15T10:30:00Z",
                false,
            )
            .expect("record");
        }

        // Insert a unique commit
        record_commit_message(
            &conn,
            repo_id,
            "unique_hash",
            "feat: unique feature",
            None,
            None,
            None,
            "2024-01-15T10:30:00Z",
            false,
        )
        .expect("record");

        let duplicates = find_duplicate_commits(&conn, None, 10).expect("query");
        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates[0].0, "fix: typo");
        assert_eq!(duplicates[0].1, 3);
    }

    #[test]
    fn commit_length_stats_works() {
        let (conn, repo_id) = db();

        // Insert some commits with varying lengths
        record_commit_message(
            &conn,
            repo_id,
            "hash1",
            "short",
            Some("This is a body."),
            None,
            None,
            "2024-01-15T10:30:00Z",
            false,
        )
        .expect("record");

        record_commit_message(
            &conn,
            repo_id,
            "hash2",
            "a much longer subject line that exceeds the normal length",
            None,
            None,
            None,
            "2024-01-15T10:30:00Z",
            false,
        )
        .expect("record");

        let stats = commit_length_stats(&conn, None).expect("query");
        assert_eq!(stats.total_commits, 2);
        assert!(stats.avg_subject_length > 0.0);
        assert_eq!(stats.body_percentage, 50.0); // 1 out of 2 has body
    }

    #[test]
    fn search_commits_fts_works() {
        let (conn, repo_id) = db();

        record_commit_message(
            &conn,
            repo_id,
            "hash1",
            "feat: implement user authentication",
            Some("Added JWT token validation"),
            None,
            None,
            "2024-01-15T10:30:00Z",
            false,
        )
        .expect("record");

        record_commit_message(
            &conn,
            repo_id,
            "hash2",
            "fix: database connection timeout",
            None,
            None,
            None,
            "2024-01-15T10:30:00Z",
            false,
        )
        .expect("record");

        // Search for "authentication"
        let results = search_commits(&conn, "authentication", 10).expect("query");
        assert_eq!(results.len(), 1);
        assert!(results[0].subject.contains("authentication"));

        // Search for "database"
        let results = search_commits(&conn, "database", 10).expect("query");
        assert_eq!(results.len(), 1);
        assert!(results[0].subject.contains("database"));

        // Search for "JWT" in body
        let results = search_commits(&conn, "JWT", 10).expect("query");
        assert_eq!(results.len(), 1);
    }
}
