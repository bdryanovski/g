-- Migration 003: review notes
-- Stores private/local code review notes keyed to a repo + file + line, with
-- optional commit attribution so the note can survive across refactors.
-- Never edit existing migration files — append a new migration for changes.

-- ─── Review notes (private, local-only) ─────────────────────────────────────
-- One row per private note left from the `g diff` TUI.  These never leave the
-- local DB; the two-bucket model separates them from public PR comments which
-- are sent straight to GitHub and not persisted here.
CREATE TABLE review_notes (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id       INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    -- Anchoring.  `path` is the repo-relative file path.  `line` is a 1-based
    -- line number in the *new* side of the diff (or the old side if
    -- `line_kind = "old"`).  When the diff is against a working tree we still
    -- use the post-change line numbering.
    path          TEXT    NOT NULL,
    line          INTEGER NOT NULL,
    line_kind     TEXT    NOT NULL DEFAULT 'new',  -- 'new' | 'old'
    -- Optional commit the note was left against.  When present, allows the
    -- note to survive line re-ordering (we can re-anchor via git blame). When
    -- NULL (e.g. unstaged diff), the note is anchored only by line number.
    commit_hash   TEXT,
    -- The note body.  Markdown by convention but stored verbatim.
    body          TEXT    NOT NULL,
    -- Author of the note (gleaned from git config user.name at insert time).
    author        TEXT,
    created_at    TEXT    NOT NULL,   -- ISO-8601 UTC
    updated_at   TEXT    NOT NULL    -- ISO-8601 UTC
);

CREATE INDEX idx_review_notes_repo  ON review_notes(repo_id);
CREATE INDEX idx_review_notes_path ON review_notes(repo_id, path);
CREATE INDEX idx_review_notes_line ON review_notes(repo_id, path, line);