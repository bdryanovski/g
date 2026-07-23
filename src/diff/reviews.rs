//! Review-comment coordination for the diff TUI.
//!
//! Two-bucket model:
//! - **Public comments** — pushed live to a GitHub PR via the `github` module
//!   (see `commands::notes::publish` — there's no abstraction layer because
//!   the publish path is the only caller and we don't want speculative
//!   interfaces sitting around).
//! - **Private notes** — stored only in the local SQLite DB via
//!   `storage::reviews` (driven by the `c` key in the diff TUI).
//!
//! Line attribution
//! ---------------
//! A review comment is anchored to a *file* + *line* + *side* triple.  The
//! diff parser already computes the old/new line number per `Line`; this
//! module packages that into a [`CommentAnchor`] and persists it via
//! `storage::reviews::insert`.  For unstaged diffs the `commit_hash` is
//! `None`; for staged or committed diffs it is the SHA the diff was computed
//! against, enabling re-anchoring via `git blame` in a future phase.

use anyhow::Result;
use rusqlite::Connection;

use super::parse::{FileDiff, Line};
use crate::storage::{self, LineKind, NewReviewNote, ReviewNoteRow};

/// Re-export of [`crate::github::CommentSide`] so callers don't need to reach
/// into the `github` module for the diff-side enum.
pub use crate::github::CommentSide;

/// Where a line anchor sits in the diff.  Owned so it can outlive the parsed
/// diff tree (e.g. be stashed in `AppState` while the user types the body).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentAnchor {
    /// Repo-relative path the comment is on.
    pub path: String,
    /// 1-based line number on the chosen side.
    pub line: i64,
    /// Which side of the diff the anchor refers to.
    pub side: CommentSide,
    /// Optional commit hash for re-anchoring.  `None` for unstaged diffs.
    pub commit_hash: Option<String>,
}

impl CommentSide {
    /// Map to the storage layer's [`LineKind`].
    pub fn as_line_kind(self) -> LineKind {
        match self {
            Self::Old => LineKind::Old,
            Self::New => LineKind::New,
        }
    }
}

/// Loose anchor computed directly from the parsed diff at the current cursor
/// row.  Enough to build a [`CommentAnchor`] once the caller has decided on a
/// commit hash for the band.
#[derive(Debug, Clone, Copy)]
pub struct RawAnchor<'a> {
    pub path: &'a str,
    pub line: i64,
    pub side: CommentSide,
}

/// Resolve which line a comment would anchor to, given the visible file at
/// `file_idx` and the cursor row `row` in that file's flat highlighted view.
///
/// Returns `None` when the row is out of range or the line has no attribution
/// (e.g. a `NoNewline` marker).
pub fn anchor_at_row<'a>(files: &'a [FileDiff], file_idx: usize, row: usize) -> Option<RawAnchor<'a>> {
    let file = files.get(file_idx)?;
    let line = locate_row(file, row)?;
    let side = match line.marker() {
        '+' => CommentSide::New,
        '-' => CommentSide::Old,
        _ => CommentSide::New, // context line: anchor to the new side by default
    };
    let line_no = match side {
        CommentSide::New => line.new_no()?,
        CommentSide::Old => line.old_no()?,
    };
    Some(RawAnchor {
        path: &file.path,
        line: line_no as i64,
        side,
    })
}

/// Build a fully-owned [`CommentAnchor`] from a [`RawAnchor`] plus a commit
/// hash band (the SHA the diff was computed against, if any).
pub fn promote_anchor(raw: RawAnchor<'_>, commit_hash: Option<String>) -> CommentAnchor {
    CommentAnchor {
        path: raw.path.to_owned(),
        line: raw.line,
        side: raw.side,
        commit_hash,
    }
}

/// Save a private review note to the local DB.
///
/// Convenience wrapper around `storage::reviews::insert` that lifts `&str`
/// into a [`NewReviewNote`] and threads the `repo_id`.
pub fn save_private_note(
    conn: &Connection,
    repo_id: i64,
    anchor: &CommentAnchor,
    body: &str,
    author: Option<&str>,
    author_email: Option<&str>,
) -> Result<i64> {
    let note = NewReviewNote {
        path: &anchor.path,
        line: anchor.line,
        line_kind: anchor.side.as_line_kind(),
        commit_hash: anchor.commit_hash.as_deref(),
        body,
        author,
        author_email,
    };
    storage::reviews::insert(conn, repo_id, &note)
}

/// Load all private notes for a single file in `repo_id`.
pub fn private_notes_for_file(
    conn: &Connection,
    repo_id: i64,
    path: &str,
) -> Result<Vec<ReviewNoteRow>> {
    storage::reviews::load_for_file(conn, repo_id, path)
}

// ─── Internal helpers ────────────────────────────────────────────────────────

/// Walk the hunks of `file` and locate the [`Line`] corresponding to the
/// flat `row` index (the same row the TUI uses for its scroll cursor).
///
/// Returns `None` when `row` is out of range.
fn locate_row(file: &FileDiff, row: usize) -> Option<&Line> {
    let mut i = 0usize;
    for hunk in &file.hunks {
        for line in &hunk.lines {
            if i == row {
                return Some(line);
            }
            i += 1;
        }
    }
    None
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::parse;
    use crate::storage::{migrations, repos};
    use rusqlite::Connection;

    fn sample_diff() -> Vec<FileDiff> {
        let raw = "\
diff --git a/foo.rs b/foo.rs
index 1111111..2222222 100644
--- a/foo.rs
+++ b/foo.rs
@@ -1,3 +1,4 @@
 fn main() {
-    let x = 1;
+    let x = 2;
+    let y = 3;
 }
";
        parse::parse(raw)
    }

    #[test]
    fn anchor_at_row_resolves_added_line() {
        let files = sample_diff();
        // row 0 = context `fn main() {`     (old_no=1, new_no=1)
        // row 1 = del   `    let x = 1;`    (old_no=2)
        // row 2 = add   `    let x = 2;`    (                  new_no=2)
        // row 3 = add   `    let y = 3;`    (                  new_no=3)
        // row 4 = context `}`               (old_no=3, new_no=4)
        let anchor = anchor_at_row(&files, 0, 2).expect("row 2 should anchor");
        assert_eq!(anchor.path, "foo.rs");
        assert_eq!(anchor.line, 2); // new_no for the first +line
        assert_eq!(anchor.side, CommentSide::New);
    }

    #[test]
    fn anchor_at_row_resolves_deleted_line() {
        let files = sample_diff();
        let anchor = anchor_at_row(&files, 0, 1).expect("row 1 should anchor");
        assert_eq!(anchor.line, 2); // 2 in old numbering
        assert_eq!(anchor.side, CommentSide::Old);
    }

    #[test]
    fn anchor_at_row_out_of_range_returns_none() {
        let files = sample_diff();
        assert!(anchor_at_row(&files, 0, 999).is_none());
        assert!(anchor_at_row(&files, 99, 0).is_none());
    }

    #[test]
    fn promote_anchor_takes_commit_hash() {
        let files = sample_diff();
        let raw = anchor_at_row(&files, 0, 2).expect("anchor");
        let anchor = promote_anchor(raw, Some("deadbeef".to_string()));
        assert_eq!(anchor.path, "foo.rs");
        assert_eq!(anchor.line, 2);
        assert_eq!(anchor.side, CommentSide::New);
        assert_eq!(anchor.commit_hash.as_deref(), Some("deadbeef"));
    }

    #[test]
    fn save_private_note_round_trips() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run(&conn).expect("migrations");
        let repo_id = repos::upsert(&conn, "/repo").expect("upsert repo");

        let anchor = CommentAnchor {
            path: "foo.rs".to_string(),
            line: 3,
            side: CommentSide::New,
            commit_hash: Some("abc123".to_string()),
        };
        let id = save_private_note(&conn, repo_id, &anchor, "nit: use 4", Some("alice"), Some("alice@example.com"))
            .expect("save");
        assert!(id > 0);

        let notes = private_notes_for_file(&conn, repo_id, "foo.rs").expect("load");
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].line, 3);
        assert_eq!(notes[0].body, "nit: use 4");
        assert_eq!(notes[0].author.as_deref(), Some("alice"));
        assert_eq!(notes[0].author_email.as_deref(), Some("alice@example.com"));
        assert_eq!(notes[0].commit_hash.as_deref(), Some("abc123"));
        assert_eq!(notes[0].line_kind, LineKind::New);
    }

    #[test]
    fn side_maps_to_line_kind() {
        assert_eq!(CommentSide::New.as_line_kind(), LineKind::New);
        assert_eq!(CommentSide::Old.as_line_kind(), LineKind::Old);
    }
}