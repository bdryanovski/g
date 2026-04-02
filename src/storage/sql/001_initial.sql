-- Migration 001: initial schema
-- Creates all tables for workspaces, stacks, and stats tracking.
-- Never edit this file — append a new migration file for changes.

-- ─── Repos ───────────────────────────────────────────────────────────────────
-- Shared anchor row referenced by all per-repo tables.
-- Keyed by the absolute filesystem path of the repository root (or container
-- root for repos using the container workspace layout).
CREATE TABLE repos (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    path       TEXT    NOT NULL UNIQUE,
    name       TEXT    NOT NULL,   -- derived: last path component
    first_seen TEXT    NOT NULL,   -- ISO-8601 UTC
    last_seen  TEXT    NOT NULL    -- ISO-8601 UTC, updated on every CLI run
);

-- ─── Workspaces ──────────────────────────────────────────────────────────────
-- Replaces workspaces.toml.
-- One row per git worktree registered with `g workspace create` (or auto-
-- discovered by reconcile_store_with_git).
CREATE TABLE workspaces (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    name            TEXT    NOT NULL,
    description     TEXT,                      -- nullable
    path            TEXT    NOT NULL UNIQUE,   -- absolute filesystem path
    branch          TEXT    NOT NULL,
    container_root  TEXT,                      -- set by `g workspace init`
    created_at      TEXT    NOT NULL,          -- ISO-8601 UTC
    UNIQUE(repo_id, name)
);

-- ─── Stacks ──────────────────────────────────────────────────────────────────
-- Replaces stacks.toml.
CREATE TABLE stacks (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id      INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    name         TEXT    NOT NULL,
    root_branch  TEXT    NOT NULL,   -- bottom/target branch of the stack
    created_at   TEXT    NOT NULL,   -- ISO-8601 UTC
    updated_at   TEXT    NOT NULL,   -- ISO-8601 UTC
    UNIQUE(repo_id, name)
);

-- Ordered branches within a stack. `position` is 0-based, bottom to top.
CREATE TABLE stack_branches (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    stack_id    INTEGER NOT NULL REFERENCES stacks(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    name        TEXT    NOT NULL,
    pr_number   INTEGER,   -- nullable
    pr_url      TEXT,      -- nullable
    description TEXT,      -- nullable
    UNIQUE(stack_id, position),
    UNIQUE(stack_id, name)
);

-- ─── Stats: Command runs ─────────────────────────────────────────────────────
-- One row per CLI invocation. Recorded unconditionally in main::run().
CREATE TABLE command_runs (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    command       TEXT    NOT NULL,   -- e.g. "commit", "workspace", "log"
    subcommand    TEXT,               -- e.g. "create" for "workspace create"
    repo_id       INTEGER REFERENCES repos(id),   -- NULL when not inside a git repo
    ran_at        TEXT    NOT NULL,   -- ISO-8601 UTC
    duration_ms   INTEGER,            -- wall-clock ms; NULL if not measured
    exit_code     INTEGER NOT NULL DEFAULT 0,
    error_message TEXT                -- populated when exit_code != 0
);

CREATE INDEX idx_command_runs_command ON command_runs(command);
CREATE INDEX idx_command_runs_ran_at  ON command_runs(ran_at);
CREATE INDEX idx_command_runs_repo_id ON command_runs(repo_id);

-- ─── Stats: Branch events ────────────────────────────────────────────────────
-- Records branch-level git operations performed through the tool.
-- event_type: 'checkout' | 'create' | 'delete' | 'push' | 'rebase'
CREATE TABLE branch_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id     INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    branch_name TEXT    NOT NULL,
    event_type  TEXT    NOT NULL,
    occurred_at TEXT    NOT NULL   -- ISO-8601 UTC
);

CREATE INDEX idx_branch_events_repo   ON branch_events(repo_id);
CREATE INDEX idx_branch_events_branch ON branch_events(branch_name);

-- ─── Stats: Commit records ───────────────────────────────────────────────────
-- One row per commit made through `g commit`.
CREATE TABLE commit_records (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id      INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    commit_type  TEXT,              -- "feat" | "fix" | "docs" | etc.
    scope        TEXT,              -- nullable
    has_body     INTEGER NOT NULL DEFAULT 0,   -- 0 or 1
    gpg_signed   INTEGER NOT NULL DEFAULT 0,   -- 0 or 1
    committed_at TEXT    NOT NULL   -- ISO-8601 UTC
);

CREATE INDEX idx_commit_records_repo ON commit_records(repo_id);
CREATE INDEX idx_commit_records_type ON commit_records(commit_type);

-- ─── Stats: Workspace events ─────────────────────────────────────────────────
-- event_type: 'create' | 'switch' | 'delete' | 'rename' | 'init' | 'clone'
CREATE TABLE workspace_events (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id INTEGER REFERENCES workspaces(id) ON DELETE SET NULL,
    repo_id      INTEGER REFERENCES repos(id) ON DELETE CASCADE,
    event_type   TEXT    NOT NULL,
    occurred_at  TEXT    NOT NULL   -- ISO-8601 UTC
);

-- ─── Stats: Stack events ─────────────────────────────────────────────────────
-- event_type: 'create' | 'add' | 'push' | 'pr' | 'squash' | 'fold' |
--             'sync' | 'absorb' | 'delete' | 'switch'
CREATE TABLE stack_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    stack_id    INTEGER REFERENCES stacks(id) ON DELETE SET NULL,
    repo_id     INTEGER REFERENCES repos(id) ON DELETE CASCADE,
    event_type  TEXT    NOT NULL,
    occurred_at TEXT    NOT NULL   -- ISO-8601 UTC
);
