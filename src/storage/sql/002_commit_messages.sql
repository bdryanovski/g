-- Migration 002: commit messages table
-- Stores full commit messages (subject + body) for statistics and search.
-- Never edit existing migration files — append a new migration for changes.

-- ─── Stats: Commit messages ──────────────────────────────────────────────────
-- One row per commit made through `g commit` or imported from git history.
-- Enables: duplicate detection, message length analysis, fuzzy search.
CREATE TABLE commit_messages (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id       INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    commit_hash   TEXT    NOT NULL,   -- full SHA-1 hash
    subject       TEXT    NOT NULL,   -- first line (max ~72 chars typically)
    body          TEXT,               -- remaining lines after blank, nullable
    author_name   TEXT,               -- commit author name
    author_email  TEXT,               -- commit author email
    committed_at  TEXT    NOT NULL,   -- ISO-8601 UTC (from git or insertion time)
    imported      INTEGER NOT NULL DEFAULT 0,  -- 1 if backfilled from git history
    UNIQUE(repo_id, commit_hash)
);

-- Indexes for efficient queries
CREATE INDEX idx_commit_messages_repo     ON commit_messages(repo_id);
CREATE INDEX idx_commit_messages_subject  ON commit_messages(subject);
CREATE INDEX idx_commit_messages_date     ON commit_messages(committed_at);
CREATE INDEX idx_commit_messages_hash     ON commit_messages(commit_hash);

-- Full-text search virtual table for fuzzy matching commit messages
-- Uses FTS5 for fast text search across subject and body
CREATE VIRTUAL TABLE commit_messages_fts USING fts5(
    subject,
    body,
    content='commit_messages',
    content_rowid='id'
);

-- Triggers to keep FTS index in sync with commit_messages table
CREATE TRIGGER commit_messages_ai AFTER INSERT ON commit_messages BEGIN
    INSERT INTO commit_messages_fts(rowid, subject, body)
    VALUES (new.id, new.subject, new.body);
END;

CREATE TRIGGER commit_messages_ad AFTER DELETE ON commit_messages BEGIN
    INSERT INTO commit_messages_fts(commit_messages_fts, rowid, subject, body)
    VALUES ('delete', old.id, old.subject, old.body);
END;

CREATE TRIGGER commit_messages_au AFTER UPDATE ON commit_messages BEGIN
    INSERT INTO commit_messages_fts(commit_messages_fts, rowid, subject, body)
    VALUES ('delete', old.id, old.subject, old.body);
    INSERT INTO commit_messages_fts(rowid, subject, body)
    VALUES (new.id, new.subject, new.body);
END;
