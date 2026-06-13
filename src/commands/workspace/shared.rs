//! Cross-subcommand helpers for the workspace module.
//!
//! These centralise four kinds of work:
//!
//! - **Live git worktree introspection** — parsing `git worktree list --porcelain`.
//! - **Repository resolution** — mapping the current directory to a `repo_id`
//!   in the SQLite store, including container-layout support.
//! - **Workspace resolution** — turning a user-typed identifier (name or
//!   branch) into either a stored row or a live git worktree.
//! - **Reconciliation** — auto-registering worktrees that exist on disk but
//!   are missing from the database, so the store self-heals.

use anyhow::{bail, Context, Result};
use chrono::Utc;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

use crate::commands::git as gitcmd;
use crate::config;
use crate::storage::{repos, workspaces as ws_store, WorkspaceRow};
use crate::ui;

// ─── Live worktree info ──────────────────────────────────────────────────────

/// Live worktree info parsed from `git worktree list --porcelain`.
pub(super) struct WorktreeInfo {
    pub path: PathBuf,
    pub head: String,
    pub branch: Option<String>,
    pub bare: bool,
}

/// The result of resolving a workspace identifier.  The identifier may match a
/// DB row (by name or branch) or only a live git worktree (by branch).
pub(super) enum ResolvedWorkspace<'a> {
    /// Matched a row in the `workspaces` table.
    DbRow(&'a WorkspaceRow),
    /// No DB row, but a live git worktree matched by branch name.
    GitOnly { path: PathBuf, branch: String },
}

/// Try to find a workspace by `query` (name first, then branch) in the DB
/// rows.  If nothing matches, fall back to the live git worktree list.
pub(super) fn resolve_workspace<'a>(
    workspaces: &'a [WorkspaceRow],
    query: &str,
) -> Result<ResolvedWorkspace<'a>> {
    // 1. Exact name match.
    if let Some(ws) = workspaces.iter().find(|w| w.name == query) {
        return Ok(ResolvedWorkspace::DbRow(ws));
    }

    // 2. Exact branch match.
    if let Some(ws) = workspaces.iter().find(|w| w.branch == query) {
        return Ok(ResolvedWorkspace::DbRow(ws));
    }

    // 3. Case-insensitive substring on name (keeps existing switch behavior).
    let lower = query.to_lowercase();
    if let Some(ws) = workspaces
        .iter()
        .find(|w| w.name.to_lowercase().contains(&lower))
    {
        return Ok(ResolvedWorkspace::DbRow(ws));
    }

    // 4. Fall back to live git worktrees — match by branch.
    let worktrees = list_worktrees()?;
    if let Some(wt) = worktrees
        .iter()
        .find(|wt| wt.branch.as_deref() == Some(query))
    {
        return Ok(ResolvedWorkspace::GitOnly {
            path: wt.path.clone(),
            branch: query.to_string(),
        });
    }

    bail!(
        "Workspace '{}' not found. Run `{} workspace list` to see all workspaces.",
        query,
        crate::bin_name()
    );
}

// ─── Repo resolution ────────────────────────────────────────────────────────

/// Resolve the SQLite `repo_id` for the current working directory.
///
/// Algorithm (mirrors the original TOML key resolution):
/// 1. Get the current git repo root via `git rev-parse --show-toplevel`.
/// 2. Query all workspace rows that have a `container_root`.  If any
///    `container_root` is a prefix of the current repo root, return that
///    workspace's `repo_id` (we are inside a known container).
/// 3. Otherwise upsert a repo row for the current repo root and return its id.
pub(super) fn current_repo_id(conn: &Connection) -> Result<i64> {
    let repo_root = gitcmd::repo_root()?;

    // Look for a known container_root that is a prefix of repo_root.
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT repo_id, container_root
             FROM workspaces WHERE container_root IS NOT NULL",
        )
        .context("Failed to prepare container lookup")?;

    let entries: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .context("Failed to query container roots")?
        .filter_map(|r| r.ok())
        .collect();

    for (id, container_root) in &entries {
        if repo_root.starts_with(container_root.as_str()) {
            return Ok(*id);
        }
    }

    repos::upsert(conn, &repo_root)
}

/// Load existing workspace rows for the current repo and reconcile with live
/// git worktrees, auto-registering any worktrees that git knows about but the
/// DB does not.
///
/// Returns `(repo_id, workspaces)`.
pub(super) fn load_repo_workspaces(conn: &Connection) -> Result<(i64, Vec<WorkspaceRow>)> {
    let repo_id = current_repo_id(conn)?;
    reconcile_with_git(conn, repo_id);
    let rows = ws_store::load_for_repo(conn, repo_id)?;
    Ok((repo_id, rows))
}

/// Compare live git worktrees against the DB and insert any missing entries.
///
/// Self-heals after crashes, failed writes, or manual `git worktree add`
/// calls.  Write failures here are non-fatal — the in-memory data is still
/// correct for this invocation.
fn reconcile_with_git(conn: &Connection, repo_id: i64) {
    let worktrees = match list_worktrees() {
        Ok(wt) => wt,
        Err(_) => return,
    };

    let container_root = ws_store::get_container_root(conn, repo_id).unwrap_or(None);

    for wt in &worktrees {
        if wt.bare {
            continue;
        }
        let path_str = wt.path.to_string_lossy().to_string();

        if ws_store::find_by_path(conn, &path_str)
            .unwrap_or(None)
            .is_some()
        {
            continue;
        }

        let branch = wt.branch.as_deref().unwrap_or("unknown").to_string();
        let name = wt
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| branch.clone());

        let new_ws = ws_store::NewWorkspace {
            name: &name,
            description: None,
            path: &path_str,
            branch: &branch,
            container_root: container_root.as_deref(),
            created_at: Utc::now(),
        };
        let _ = ws_store::insert(conn, repo_id, &new_ws);
    }
}

// ─── Git worktree helpers ────────────────────────────────────────────────────

/// Query git for all worktree entries and parse the `--porcelain` format.
pub(super) fn list_worktrees() -> Result<Vec<WorktreeInfo>> {
    let raw = gitcmd::git_output(&["worktree", "list", "--porcelain"])
        .context("Failed to list git worktrees. Are you inside a git repository?")?;

    let mut worktrees = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_head = String::new();
    let mut current_branch: Option<String> = None;
    let mut is_bare = false;

    for line in raw.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            if let Some(path) = current_path.take() {
                worktrees.push(WorktreeInfo {
                    path,
                    head: current_head.clone(),
                    branch: current_branch.take(),
                    bare: is_bare,
                });
            }
            current_path = Some(PathBuf::from(p));
            current_head = String::new();
            current_branch = None;
            is_bare = false;
        } else if let Some(h) = line.strip_prefix("HEAD ") {
            current_head = h.to_string();
        } else if let Some(b) = line.strip_prefix("branch ") {
            current_branch = Some(b.strip_prefix("refs/heads/").unwrap_or(b).to_string());
        } else if line == "bare" {
            is_bare = true;
        }
    }

    if let Some(path) = current_path.take() {
        worktrees.push(WorktreeInfo {
            path,
            head: current_head,
            branch: current_branch,
            bare: is_bare,
        });
    }

    Ok(worktrees)
}

/// Compute the filesystem path for a new worktree named `name`.
///
/// Uses the container layout when a `container_root` exists for the current
/// repo; otherwise falls back to the sibling layout.
pub(super) fn worktree_path_for(conn: &Connection, name: &str) -> Result<PathBuf> {
    let repo_id = current_repo_id(conn)?;

    if let Some(container) = ws_store::get_container_root(conn, repo_id)? {
        return Ok(PathBuf::from(container).join(name));
    }

    let cfg = config::load()?;
    let repo_root = gitcmd::repo_root()?;
    let root = Path::new(&repo_root);
    let repo_name = root
        .file_name()
        .context("Could not determine repo directory name")?
        .to_string_lossy();
    let parent = root.parent().context("Repo is at filesystem root")?;
    let dir_name = format!("{}{}{}", repo_name, cfg.workspace.separator, name);
    Ok(parent.join(dir_name))
}

/// Render a human-readable "time ago" string.
pub(super) fn format_relative_time(dt: chrono::DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(dt);
    let relative = if diff.num_days() > 365 {
        format!("{} years ago", diff.num_days() / 365)
    } else if diff.num_days() > 30 {
        format!("{} months ago", diff.num_days() / 30)
    } else if diff.num_days() > 0 {
        format!("{} days ago", diff.num_days())
    } else if diff.num_hours() > 0 {
        format!("{} hours ago", diff.num_hours())
    } else {
        format!("{} min ago", diff.num_minutes().max(1))
    };
    ui::muted(&relative)
}
