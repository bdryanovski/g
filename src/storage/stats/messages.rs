//! Read-side queries against the `commit_messages` table — full-text search,
//! duplicate detection, length statistics, monthly trends and top authors.

use anyhow::Result;
use chrono::Utc;
use rusqlite::Connection;

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
pub fn commit_length_stats(conn: &Connection, repo_id: Option<i64>) -> Result<CommitLengthStats> {
    let where_clause = repo_id.map(|_| "WHERE repo_id = ?1").unwrap_or("");

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
                if repo_id.is_some() {
                    "WHERE repo_id = ?1"
                } else {
                    ""
                }
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
                if repo_id.is_some() {
                    "WHERE repo_id = ?1"
                } else {
                    ""
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::stats::events::record_commit_message;
    use crate::storage::{migrations, repos};

    fn db() -> (Connection, i64) {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run(&conn).expect("migrations");
        let repo_id = repos::upsert(&conn, "/home/user/repo").expect("repo");
        (conn, repo_id)
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
