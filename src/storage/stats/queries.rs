//! Read-side aggregation queries for the `g stats` report.
//!
//! All queries are read-only and tolerant of missing or empty tables: a query
//! that fails (e.g. a brand-new database) returns the zero value rather than
//! an error so the rest of the report keeps rendering.

use anyhow::Result;
use chrono::{NaiveDate, Utc};
use rusqlite::Connection;

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
