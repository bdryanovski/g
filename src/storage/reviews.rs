//! Review-note persistence (the "private" bucket of the two-bucket model).
//!
//! Stores local-only code-review notes keyed to a repo + file + line.  The
//! "public" bucket — comments pushed to a GitHub PR — is handled in the
//! `github` module and never persisted here; only the user's private notes
//! live in SQLite.
//!
//! Anchoring model
//! ---------------
//! A note has a `path`, a 1-based `line`, and a `line_kind` (`"new"` or
//! `"old"`).  When the diff the note was left against has a known commit
//! (staged or committed diffs), `commit_hash` is recorded so the note can be
//! re-anchored across refactors via `git blame`.  For unstaged diffs
//! `commit_hash` is NULL and the note is anchored only by line number.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::Connection;

// ─── Types ───────────────────────────────────────────────────────────────────

/// Which side of the diff a review note is anchored to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Post-change line numbering (the `+` side, or the "after" state).
    New,
    /// Pre-change line numbering (the `-` side, or the "before" state).
    Old,
}

impl LineKind {
    /// Render as the string stored in the DB.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Old => "old",
        }
    }

    /// Parse from the DB string; unknown values fall back to [`LineKind::New`]
    /// with a warning logged to stderr.
    fn from_str(s: &str) -> Self {
        match s {
            "new" => Self::New,
            "old" => Self::Old,
            other => {
                eprintln!(
                    "Warning: Unknown line_kind '{}' in review_notes table, defaulting to 'new'",
                    other
                );
                Self::New
            }
        }
    }
}

/// A row from the `review_notes` table.
#[derive(Debug, Clone)]
pub struct ReviewNoteRow {
    /// Primary key.
    pub id: i64,
    /// FK to `repos(id)`.
    pub repo_id: i64,
    /// Repo-relative file path the note is attached to.
    pub path: String,
    /// 1-based line number on the chosen side.
    pub line: i64,
    /// Which side of the diff the line refers to.
    pub line_kind: LineKind,
    /// Optional commit hash (full SHA-1) the note was left against.
    pub commit_hash: Option<String>,
    /// The note body, verbatim (Markdown by convention).
    pub body: String,
    /// Author name from git config at insert time.
    pub author: Option<String>,
    /// Author email from git config at insert time.
    pub author_email: Option<String>,
    /// When the note was created.
    pub created_at: DateTime<Utc>,
    /// When the note was last edited.
    pub updated_at: DateTime<Utc>,
}

/// Input type for [`insert`] — fields mirror [`ReviewNoteRow`] minus `id`,
/// `created_at`, and `updated_at`, which are set by the database.
pub struct NewReviewNote<'a> {
    pub path: &'a str,
    pub line: i64,
    pub line_kind: LineKind,
    pub commit_hash: Option<&'a str>,
    pub body: &'a str,
    pub author: Option<&'a str>,
    pub author_email: Option<&'a str>,
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Load all review notes for `repo_id`, ordered by path then line.
///
/// Useful for the `g diff --list-notes` summary view and for re-anchoring.
pub fn load_for_repo(conn: &Connection, repo_id: i64) -> Result<Vec<ReviewNoteRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, repo_id, path, line, line_kind, commit_hash, body, author,
                    author_email, created_at, updated_at
             FROM review_notes
             WHERE repo_id = ?1
             ORDER BY path ASC, line ASC",
        )
        .context("Failed to prepare review_notes query")?;

    let rows = stmt
        .query_map(rusqlite::params![repo_id], map_row)
        .context("Failed to query review_notes")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to read review_notes rows")?;

    Ok(rows)
}

/// Load all notes for a specific file in `repo_id`, ordered by line.
pub fn load_for_file(conn: &Connection, repo_id: i64, path: &str) -> Result<Vec<ReviewNoteRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, repo_id, path, line, line_kind, commit_hash, body, author,
                    author_email, created_at, updated_at
             FROM review_notes
             WHERE repo_id = ?1 AND path = ?2
             ORDER BY line ASC",
        )
        .context("Failed to prepare review_notes file query")?;

    let rows = stmt
        .query_map(rusqlite::params![repo_id, path], map_row)
        .context("Failed to query review_notes by file")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to read review_notes rows")?;

    Ok(rows)
}

/// Load a single note by primary key.
///
/// Returns `Ok(None)` when no row with that `id` exists.
pub fn load_by_id(conn: &Connection, id: i64) -> Result<Option<ReviewNoteRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, repo_id, path, line, line_kind, commit_hash, body, author,
                    author_email, created_at, updated_at
             FROM review_notes
             WHERE id = ?1",
        )
        .context("Failed to prepare review_notes by-id query")?;

    let mut rows = stmt
        .query_map(rusqlite::params![id], map_row)
        .context("Failed to query review_notes by id")?;

    match rows.next() {
        Some(Ok(row)) => Ok(Some(row)),
        Some(Err(e)) => Err(e).context("Failed to read review_notes row"),
        None => Ok(None),
    }
}

/// Insert a new review note and return its primary key.
pub fn insert(conn: &Connection, repo_id: i64, note: &NewReviewNote<'_>) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO review_notes
             (repo_id, path, line, line_kind, commit_hash, body, author, author_email,
              created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        rusqlite::params![
            repo_id,
            note.path,
            note.line,
            note.line_kind.as_str(),
            note.commit_hash,
            note.body,
            note.author,
            note.author_email,
            now,
        ],
    )
    .with_context(|| {
        format!("Failed to insert review note for {}:{}", note.path, note.line)
    })?;

    Ok(conn.last_insert_rowid())
}

/// Update the body (and refresh `updated_at`) of an existing note.
pub fn update_body(conn: &Connection, id: i64, body: &str) -> Result<()> {
    conn.execute(
        "UPDATE review_notes SET body = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![body, Utc::now().to_rfc3339(), id],
    )
    .context("Failed to update review note body")?;
    Ok(())
}

/// Delete a single note by id.
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM review_notes WHERE id = ?1",
        rusqlite::params![id],
    )
    .context("Failed to delete review note")?;
    Ok(())
}

/// Delete all notes anchored to `path` in `repo_id`.  Returns the count removed.
pub fn delete_for_file(conn: &Connection, repo_id: i64, path: &str) -> Result<usize> {
    let n = conn.execute(
        "DELETE FROM review_notes WHERE repo_id = ?1 AND path = ?2",
        rusqlite::params![repo_id, path],
    )
    .context("Failed to delete review notes for file")?;
    Ok(n)
}

/// Delete a note at a specific location (path + line + line_kind).
pub fn delete_by_location(
    conn: &Connection,
    repo_id: i64,
    path: &str,
    line: i64,
    line_kind: LineKind,
) -> Result<usize> {
    let n = conn.execute(
        "DELETE FROM review_notes WHERE repo_id = ?1 AND path = ?2 AND line = ?3 AND line_kind = ?4",
        rusqlite::params![repo_id, path, line, line_kind.as_str()],
    )
    .context("Failed to delete review note by location")?;
    Ok(n)
}

/// Delete all notes belonging to `repo_id`.  Returns the count removed.
pub fn delete_all_for_repo(conn: &Connection, repo_id: i64) -> Result<usize> {
    let n = conn
        .execute(
            "DELETE FROM review_notes WHERE repo_id = ?1",
            rusqlite::params![repo_id],
        )
        .context("Failed to clear all notes for repo")?;
    Ok(n)
}

// ─── Internal helpers ────────────────────────────────────────────────────────

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReviewNoteRow> {
    let created_at_str: String = row.get(9)?;
    let updated_at_str: String = row.get(10)?;
    let created_at = parse_ts(&created_at_str);
    let updated_at = parse_ts(&updated_at_str);
    let line_kind_str: String = row.get(4)?;
    let commit_hash: Option<String> = row.get(5)?;

    Ok(ReviewNoteRow {
        id: row.get(0)?,
        repo_id: row.get(1)?,
        path: row.get(2)?,
        line: row.get(3)?,
        line_kind: LineKind::from_str(&line_kind_str),
        commit_hash,
        body: row.get(6)?,
        author: row.get(7)?,
        author_email: row.get(8)?,
        created_at,
        updated_at,
    })
}

fn parse_ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
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
        let repo_id = repos::upsert(&conn, "/home/user/myapp").expect("upsert repo");
        (conn, repo_id)
    }

    fn sample<'a>(path: &'a str, line: i64, body: &'a str) -> NewReviewNote<'a> {
        NewReviewNote {
            path,
            line,
            line_kind: LineKind::New,
            commit_hash: None,
            body,
            author: Some("test"),
            author_email: Some("test@example.com"),
        }
    }

    #[test]
    fn insert_loads_back() {
        let (conn, repo_id) = db();
        let id = insert(&conn, repo_id, &sample("src/main.rs", 10, "nit"))
            .expect("insert");
        assert!(id > 0);
        let rows = load_for_repo(&conn, repo_id).expect("load");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].path, "src/main.rs");
        assert_eq!(rows[0].line, 10);
        assert_eq!(rows[0].body, "nit");
        assert_eq!(rows[0].author.as_deref(), Some("test"));
    }

    #[test]
    fn load_for_file_filters_path() {
        let (conn, repo_id) = db();
        insert(&conn, repo_id, &sample("src/a.rs", 1, "a")).expect("insert");
        insert(&conn, repo_id, &sample("src/b.rs", 1, "b")).expect("insert");
        let only_a = load_for_file(&conn, repo_id, "src/a.rs").expect("load");
        assert_eq!(only_a.len(), 1);
        assert_eq!(only_a[0].body, "a");
    }

    #[test]
    fn update_body_changes_value() {
        let (conn, repo_id) = db();
        let id = insert(&conn, repo_id, &sample("src/x.rs", 5, "old")).expect("insert");
        update_body(&conn, id, "new").expect("update");
        let rows = load_for_repo(&conn, repo_id).expect("load");
        assert_eq!(rows[0].body, "new");
    }

    #[test]
    fn delete_removes_row() {
        let (conn, repo_id) = db();
        let id = insert(&conn, repo_id, &sample("src/y.rs", 3, "x")).expect("insert");
        delete(&conn, id).expect("delete");
        assert!(load_for_repo(&conn, repo_id).expect("load").is_empty());
    }

    #[test]
    fn delete_for_file_removes_only_that_file() {
        let (conn, repo_id) = db();
        insert(&conn, repo_id, &sample("src/a.rs", 1, "a")).expect("insert");
        insert(&conn, repo_id, &sample("src/b.rs", 1, "b")).expect("insert");
        let n = delete_for_file(&conn, repo_id, "src/a.rs").expect("delete");
        assert_eq!(n, 1);
        assert_eq!(load_for_repo(&conn, repo_id).expect("load").len(), 1);
    }

    #[test]
    fn line_kind_round_trips() {
        assert_eq!(LineKind::New.as_str(), "new");
        assert_eq!(LineKind::Old.as_str(), "old");
        let (conn, repo_id) = db();
        let note = NewReviewNote {
            path: "src/z.rs",
            line: 1,
            line_kind: LineKind::Old,
            commit_hash: Some("abc123"),
            body: "old-side note",
            author: None,
            author_email: None,
        };
        let id = insert(&conn, repo_id, &note).expect("insert");
        let _ = id;
        let rows = load_for_repo(&conn, repo_id).expect("load");
        assert_eq!(rows[0].line_kind, LineKind::Old);
        assert_eq!(rows[0].commit_hash.as_deref(), Some("abc123"));
    }
}