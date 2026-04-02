//! Workspace (git worktree) management.
//!
//! ## Overview
//!
//! A "workspace" is a git worktree plus a small metadata record stored in the
//! `workspaces` table of `~/.config/g/g.db`.  Git is the source of truth for
//! which worktrees exist on disk; this module adds UI metadata (friendly name,
//! description, creation timestamp).
//!
//! The store is keyed by repository ID (an auto-increment integer anchored on
//! the repo's absolute root path) so metadata from multiple repositories
//! coexists in the same database without colliding.  For repos using the
//! container layout (set by `init` or `clone --workspace`), the key is the
//! container directory's repo row — which remains stable after the directory
//! reorganisation.

use anyhow::{bail, Context, Result};
use chrono::Utc;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, MultiSelect};
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::commands::git as gitcmd;
use crate::config;
use crate::storage::{repos, stats, workspaces as ws_store, WorkspaceRow};
use crate::ui;

// ─── Internal: live worktree info ────────────────────────────────────────────

/// Live worktree info parsed from `git worktree list --porcelain`.
struct WorktreeInfo {
    path: PathBuf,
    head: String,
    branch: Option<String>,
    bare: bool,
}

// ─── Internal: repo resolution ────────────────────────────────────────────────

/// Resolve the SQLite `repo_id` for the current working directory.
///
/// Algorithm (mirrors the original TOML key resolution):
/// 1. Get the current git repo root via `git rev-parse --show-toplevel`.
/// 2. Query all workspace rows that have a `container_root`.  If any
///    `container_root` is a prefix of the current repo root, return that
///    workspace's `repo_id` (we are inside a known container).
/// 3. Otherwise upsert a repo row for the current repo root and return its id.
///
/// # Errors
///
/// Returns an error if `git rev-parse` fails (not inside a git repo).
fn current_repo_id(conn: &Connection) -> Result<i64> {
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
///
/// # Errors
///
/// Propagates errors from [`current_repo_id`] or storage operations.
fn load_repo_workspaces(conn: &Connection) -> Result<(i64, Vec<WorkspaceRow>)> {
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

// ─── Internal: git worktree helpers ──────────────────────────────────────────

/// Query git for all worktree entries and parse the `--porcelain` format.
///
/// # Errors
///
/// Returns an error if `git worktree list` fails (e.g. not in a git repo).
fn list_worktrees() -> Result<Vec<WorktreeInfo>> {
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
///
/// # Errors
///
/// Returns an error if the repo root or config cannot be determined.
fn worktree_path_for(conn: &Connection, name: &str) -> Result<PathBuf> {
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
fn format_relative_time(dt: chrono::DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(dt);
    if diff.num_days() > 365 {
        format!("{} years ago", diff.num_days() / 365)
    } else if diff.num_days() > 30 {
        format!("{} months ago", diff.num_days() / 30)
    } else if diff.num_days() > 0 {
        format!("{} days ago", diff.num_days())
    } else if diff.num_hours() > 0 {
        format!("{} hours ago", diff.num_hours())
    } else {
        format!("{} min ago", diff.num_minutes().max(1))
    }
    .bright_black()
    .to_string()
}

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Reorganise an existing, non-worktree repo into the container layout.
///
/// Given a repo at `/path/to/repo` on branch `main`, the operation is:
///
/// ```text
/// mv  /path/to/repo          /path/to/repo--ws-tmp  (temp rename)
/// mkdir                      /path/to/repo           (container)
/// mv  /path/to/repo--ws-tmp  /path/to/repo/main      (inner worktree)
/// ```
///
/// The store is updated before `git worktree repair` runs so that a repair
/// failure does not leave the filesystem and metadata out of sync.
///
/// # Errors
///
/// Returns an error if:
/// - The current directory is not inside a git repository.
/// - The repo is already in container layout for this repository.
/// - The user cancels the confirmation prompt.
/// - Any filesystem or git operation fails.
pub fn init(conn: &Connection) -> Result<()> {
    let repo_root_str =
        gitcmd::repo_root().context("Not inside a git repository. Run `git init` first.")?;
    let repo_root = PathBuf::from(&repo_root_str);

    let parent = repo_root
        .parent()
        .context("Repository is at the filesystem root — cannot create container.")?;
    let repo_name = repo_root
        .file_name()
        .context("Could not determine repository name from path")?
        .to_string_lossy()
        .to_string();

    // Detect the default branch name.
    let branch_name = gitcmd::git_output(&["symbolic-ref", "--short", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| {
            config::load()
                .map(|c| c.general.default_branch.clone())
                .unwrap_or_else(|_| "main".to_string())
        });

    // Guard: if the current repo is already inside a known container, block.
    let repo_id = current_repo_id(conn)?;
    if ws_store::get_container_root(conn, repo_id)?.is_some() {
        bail!(
            "This repository is already using the container workspace layout.\n\
             Run `{} workspace list` to see your workspaces.",
            crate::bin_name()
        );
    }

    let container_dir = repo_root.clone();
    let inner_dir = container_dir.join(&branch_name);
    let temp_name = format!("{}--ws-tmp", repo_name);
    let temp_path = parent.join(&temp_name);

    if temp_path.exists() {
        bail!(
            "Temporary path '{}' already exists. Please remove it and try again.",
            temp_path.display()
        );
    }

    // Show the plan.
    ui::print_blank();
    println!("  {} workspace init", crate::bin_name().green().bold());
    ui::print_blank();
    println!(
        "  This will reorganise '{}' into a container layout:",
        repo_root.display().to_string().cyan()
    );
    ui::print_blank();
    println!(
        "    {} {}  →  {}",
        "move".bright_black(),
        repo_root.display().to_string().yellow(),
        temp_path.display().to_string().bright_black()
    );
    println!(
        "    {} {}",
        "mkdir".bright_black(),
        container_dir.display().to_string().yellow()
    );
    println!(
        "    {} {}  →  {}",
        "move".bright_black(),
        temp_path.display().to_string().bright_black(),
        inner_dir.display().to_string().green()
    );
    ui::print_blank();
    println!(
        "  After this, new workspaces will be created inside '{}'.",
        container_dir.display().to_string().cyan()
    );
    ui::print_blank();

    if gitcmd::is_dry_run() {
        gitcmd::dry_run_action(
            &format!("mv {} {}", repo_root.display(), temp_path.display()),
            "Move repo to temporary location",
        );
        gitcmd::dry_run_action(
            &format!("mkdir {}", container_dir.display()),
            "Create container directory",
        );
        gitcmd::dry_run_action(
            &format!("mv {} {}", temp_path.display(), inner_dir.display()),
            "Move repo into container as the main workspace",
        );
        gitcmd::dry_run_action(
            "git -C <inner> worktree repair && git -C <inner> worktree prune",
            "Repair and prune git worktree tracking after directory move",
        );
        return Ok(());
    }

    let confirmed = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Proceed?")
        .default(false)
        .interact()
        .context("Confirmation prompt failed")?;

    if !confirmed {
        ui::print_blank();
        ui::print_info("Cancelled.");
        ui::print_blank();
        return Ok(());
    }

    // Step out of the repo before we move it.
    std::env::set_current_dir(parent)
        .context("Failed to change directory to parent before moving repo")?;

    // Step 1: rename repo → temp.
    fs::rename(&repo_root, &temp_path).with_context(|| {
        format!(
            "Failed to move '{}' to '{}'",
            repo_root.display(),
            temp_path.display()
        )
    })?;

    // Steps 2 + 3: create container, move temp → inner.
    let move_result = (|| -> Result<()> {
        fs::create_dir_all(&container_dir).with_context(|| {
            format!(
                "Failed to create container directory '{}'",
                container_dir.display()
            )
        })?;
        fs::rename(&temp_path, &inner_dir).with_context(|| {
            format!(
                "Failed to move '{}' to '{}'",
                temp_path.display(),
                inner_dir.display()
            )
        })?;
        Ok(())
    })();

    if let Err(e) = move_result {
        if temp_path.exists() && !repo_root.exists() {
            let _ = fs::rename(&temp_path, &repo_root);
        }
        return Err(e).context("Init failed — attempted to roll back to original state");
    }

    let container_root_str = container_dir.to_string_lossy().to_string();
    let inner_dir_str = inner_dir.to_string_lossy().to_string();

    // Persist metadata BEFORE the repair step so a repair failure does not
    // leave the filesystem and metadata out of sync.
    ws_store::set_container_root(conn, repo_id, &container_root_str)?;

    if ws_store::find_by_path(conn, &inner_dir_str)?.is_none() {
        ws_store::insert(
            conn,
            repo_id,
            &ws_store::NewWorkspace {
                name: &branch_name,
                description: None,
                path: &inner_dir_str,
                branch: &branch_name,
                container_root: Some(&container_root_str),
                created_at: Utc::now(),
            },
        )?;
    }

    // Step 4: repair git worktree tracking after the directory move.
    gitcmd::git_output(&["-C", &inner_dir_str, "worktree", "repair"])
        .context("Failed to repair git worktree tracking after move")?;
    let _ = gitcmd::git_output(&["-C", &inner_dir_str, "worktree", "prune"]);

    stats::record_workspace_event(conn, None, Some(repo_id), "init").ok();

    ui::print_blank();
    ui::print_success(&format!(
        "Repository reorganised into container layout at {}",
        container_dir.display().to_string().cyan()
    ));
    ui::print_key_value_pairs(&[
        (
            "main workspace",
            inner_dir_str.cyan().underline().to_string(),
        ),
        (
            "next",
            format!("cd {}", inner_dir.display().to_string().green()),
        ),
    ]);
    ui::print_blank();

    Ok(())
}

/// Clone a remote repository and set it up in the container/worktree layout.
///
/// 1. Queries the remote for its default branch via `git ls-remote --symref`.
/// 2. Creates a container directory named after the repo.
/// 3. Clones into `<container>/<default_branch>`.
///
/// Extra flags in `args` (`--depth`, `-q`, etc.) are forwarded to `git clone`.
///
/// # Errors
///
/// Returns an error if:
/// - No URL is found in `args`.
/// - The container directory already exists.
/// - `git ls-remote` or `git clone` fails.
/// - The store cannot be saved.
pub fn clone_with_workspace(conn: &Connection, args: &[String]) -> Result<()> {
    let url = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .with_context(|| "No URL found. Usage: g clone <url> [dest] --workspace")?;

    let repo_name = url
        .trim_end_matches('/')
        .split('/')
        .next_back()
        .unwrap_or("repo")
        .trim_end_matches(".git")
        .to_string();

    let non_flag_args: Vec<&str> = args
        .iter()
        .filter(|a| !a.starts_with('-'))
        .map(|s| s.as_str())
        .collect();
    let container_name = non_flag_args.get(1).copied().unwrap_or(repo_name.as_str());

    let cwd = std::env::current_dir()?;
    let container_dir = cwd.join(container_name);

    if container_dir.exists() {
        bail!(
            "Directory '{}' already exists. Remove it or use a different destination.",
            container_dir.display()
        );
    }

    let default_branch = detect_remote_default_branch(url).unwrap_or_else(|_| "main".to_string());

    let inner_dir = container_dir.join(&default_branch);
    let inner_dir_str = inner_dir.to_string_lossy().to_string();

    fs::create_dir_all(&container_dir).with_context(|| {
        format!(
            "Failed to create container directory '{}'",
            container_dir.display()
        )
    })?;

    let mut clone_args: Vec<&str> = vec!["clone"];
    for a in args {
        if a.starts_with('-') {
            clone_args.push(a.as_str());
        }
    }
    clone_args.push(url.as_str());
    clone_args.push(&inner_dir_str);

    let result = gitcmd::git_mutate(
        &clone_args,
        &format!("Clone '{}' into '{}'", url, inner_dir_str),
    );

    if let Err(e) = result {
        let _ = fs::remove_dir_all(&container_dir);
        return Err(e).context("Clone failed");
    }

    if !gitcmd::is_dry_run() {
        let container_root_str = container_dir.to_string_lossy().to_string();
        let repo_id = repos::upsert(conn, &container_root_str)?;

        ws_store::set_container_root(conn, repo_id, &container_root_str)?;

        if ws_store::find_by_path(conn, &inner_dir_str)?.is_none() {
            ws_store::insert(
                conn,
                repo_id,
                &ws_store::NewWorkspace {
                    name: &default_branch,
                    description: None,
                    path: &inner_dir_str,
                    branch: &default_branch,
                    container_root: Some(&container_root_str),
                    created_at: Utc::now(),
                },
            )?;
        }

        stats::record_workspace_event(conn, None, Some(repo_id), "clone").ok();

        ui::print_blank();
        ui::print_success(&format!(
            "Cloned '{}' into container workspace at {}",
            url,
            container_dir.display().to_string().cyan()
        ));
        println!(
            "     {} {}",
            "main workspace:".bright_black(),
            inner_dir_str.cyan().underline()
        );
        ui::print_blank();
        println!(
            "  {} cd {}",
            "next:".bright_black(),
            inner_dir.display().to_string().green()
        );
        ui::print_blank();
    }

    Ok(())
}

/// Query the remote for its default branch via `git ls-remote --symref`.
///
/// # Errors
///
/// Returns an error if `git ls-remote` fails or the output cannot be parsed.
fn detect_remote_default_branch(url: &str) -> Result<String> {
    let output = gitcmd::git_output(&["ls-remote", "--symref", url, "HEAD"])
        .context("Failed to query remote default branch")?;

    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("ref: refs/heads/") {
            let branch = rest.split('\t').next().unwrap_or("").trim();
            if !branch.is_empty() {
                return Ok(branch.to_string());
            }
        }
    }

    bail!("Could not determine default branch from remote HEAD")
}

/// List all git worktrees with their optional metadata.
///
/// # Errors
///
/// Returns an error if the store or git worktree listing cannot be read.
pub fn list(conn: &Connection) -> Result<()> {
    let (repo_id, workspaces) = load_repo_workspaces(conn)?;
    let _ = repo_id;
    let worktrees = list_worktrees()?;
    let cwd = std::env::current_dir()?;

    if worktrees.is_empty() {
        ui::print_blank();
        ui::print_info("No worktrees found.");
        ui::print_blank();
        return Ok(());
    }

    ui::print_blank();
    let mut table = ui::Table::new(vec!["", "Name", "Branch", "Path", "HEAD", "Created"]);

    for wt in &worktrees {
        if wt.bare {
            continue;
        }

        let branch_display = wt.branch.as_deref().unwrap_or("(detached)");
        let is_current = cwd.starts_with(&wt.path);

        let head_display = if wt.head.len() >= 7 {
            wt.head[..7].bright_black().to_string()
        } else {
            wt.head.bright_black().to_string()
        };

        let meta = workspaces.iter().find(|ws| Path::new(&ws.path) == wt.path);

        let (name_display, created_display) = if let Some(ws) = meta {
            let name = if is_current {
                ws.name.green().bold().to_string()
            } else {
                ws.name.white().to_string()
            };
            let label = match &ws.description {
                Some(desc) if !desc.is_empty() => {
                    format!("{}  {}", name, desc.bright_black())
                }
                _ => name,
            };
            (label, format_relative_time(ws.created_at))
        } else {
            let name = if is_current {
                "(main)".green().bold().to_string()
            } else {
                "(main)".bright_black().to_string()
            };
            (name, "\u{2014}".bright_black().to_string())
        };

        let marker = if is_current {
            "\u{25c9}".green().bold().to_string()
        } else {
            "\u{25ef}".bright_black().to_string()
        };

        table.add_row(vec![
            marker,
            name_display,
            ui::color_branch(branch_display),
            wt.path.display().to_string().bright_black().to_string(),
            head_display,
            created_display,
        ]);
    }

    table.print();
    ui::print_blank();

    if workspaces.is_empty() {
        ui::print_tip(&format!(
            "{} workspace create <name>  to create a worktree workspace",
            crate::bin_name()
        ));
        ui::print_blank();
    }

    Ok(())
}

/// Create a new worktree and save its metadata.
///
/// Branch resolution order:
/// 1. Branch exists locally → check out directly.
/// 2. Branch exists on `origin` → create a local tracking branch.
/// 3. Neither → create a new branch (optionally from `start_point`).
///
/// When `copy` is `true`, an interactive picker lets the user choose
/// untracked and gitignored files to copy into the new worktree.
///
/// # Errors
///
/// Returns an error if:
/// - A workspace with the same name already exists in the current repo.
/// - The computed directory already exists on disk.
/// - `git worktree add` fails.
/// - The file-copy step fails (when `copy` is `true`).
pub fn create(
    conn: &Connection,
    name: &str,
    branch: Option<&str>,
    start_point: Option<&str>,
    description: Option<&str>,
    copy: bool,
) -> Result<()> {
    let (repo_id, workspaces) = load_repo_workspaces(conn)?;

    if workspaces.iter().any(|w| w.name == name) {
        bail!("Workspace '{}' already exists. Use a different name.", name);
    }

    let wt_path = worktree_path_for(conn, name)?;
    if wt_path.exists() {
        bail!(
            "Directory '{}' already exists. Choose a different workspace name.",
            wt_path.display()
        );
    }

    let branch_name = branch.unwrap_or(name);
    let wt_path_str = wt_path.to_string_lossy().to_string();

    let local_exists = gitcmd::git_output(&["rev-parse", "--verify", branch_name]).is_ok();
    let remote_ref = format!("origin/{}", branch_name);
    let remote_exists =
        !local_exists && gitcmd::git_output(&["rev-parse", "--verify", &remote_ref]).is_ok();

    let result = if local_exists {
        gitcmd::git_mutate(
            &["worktree", "add", &wt_path_str, branch_name],
            &format!(
                "Create worktree for existing local branch '{}' at {}",
                branch_name, wt_path_str
            ),
        )
    } else if remote_exists {
        gitcmd::git_mutate(
            &[
                "worktree",
                "add",
                &wt_path_str,
                "-b",
                branch_name,
                "--track",
                &remote_ref,
            ],
            &format!(
                "Create worktree tracking remote branch '{}' at {}",
                branch_name, wt_path_str
            ),
        )
    } else {
        let mut args = vec!["worktree", "add", "-b", branch_name, &wt_path_str];
        if let Some(sp) = start_point {
            args.push(sp);
        }
        gitcmd::git_mutate(
            &args,
            &format!(
                "Create new branch '{}' and worktree at {}",
                branch_name, wt_path_str
            ),
        )
    };

    result.with_context(|| {
        format!(
            "Failed to create worktree at '{}' for branch '{}'",
            wt_path.display(),
            branch_name
        )
    })?;

    if !gitcmd::is_dry_run() {
        let container_root = ws_store::get_container_root(conn, repo_id)?;
        let ws_id = ws_store::insert(
            conn,
            repo_id,
            &ws_store::NewWorkspace {
                name,
                description,
                path: &wt_path_str,
                branch: branch_name,
                container_root: container_root.as_deref(),
                created_at: Utc::now(),
            },
        )?;
        stats::record_workspace_event(conn, Some(ws_id), Some(repo_id), "create").ok();
        stats::record_branch_event(conn, repo_id, branch_name, "create").ok();
    } else {
        gitcmd::dry_run_action(
            "Save workspace metadata",
            &format!(
                "Register workspace '{}' on branch '{}' in g.db",
                name, branch_name
            ),
        );
    }

    if copy && !gitcmd::is_dry_run() {
        copy_untracked_files(&wt_path)?;
    } else if copy {
        gitcmd::dry_run_action(
            "Copy untracked files",
            "Show interactive picker and copy selected files to new worktree",
        );
    }

    if !gitcmd::is_dry_run() {
        ui::print_blank();
        ui::print_success(&format!(
            "Created workspace {} on branch {}",
            name.green().bold(),
            branch_name.cyan()
        ));
        ui::print_key_value_pairs(&[("path", wt_path_str.cyan().underline().to_string())]);
        ui::print_blank();
        ui::print_tip(&format!(
            "{} workspace switch {}  to open a shell there",
            crate::bin_name(),
            name
        ));
        ui::print_blank();
    }

    Ok(())
}

/// Present an interactive checklist of untracked and gitignored files and copy
/// the user's selection into `dest`.
fn copy_untracked_files(dest: &Path) -> Result<()> {
    let untracked_out =
        gitcmd::git_output(&["ls-files", "--others", "--exclude-standard"]).unwrap_or_default();

    let ignored_out = gitcmd::git_output(&[
        "ls-files",
        "--others",
        "-i",
        "--exclude-standard",
        "--directory",
    ])
    .unwrap_or_default();

    let mut candidates: Vec<String> = untracked_out
        .lines()
        .chain(ignored_out.lines())
        .filter(|l| !l.is_empty())
        .map(|l| l.trim_end_matches('/').to_string())
        .collect();
    candidates.dedup();

    if candidates.is_empty() {
        return Ok(());
    }

    ui::print_blank();
    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt(
            "Select files to copy into the new workspace (space to toggle, enter to confirm)",
        )
        .items(&candidates)
        .interact_opt()
        .context("File picker failed")?;

    let selected = match selections {
        None => return Ok(()),
        Some(v) if v.is_empty() => return Ok(()),
        Some(v) => v,
    };

    let repo_root = gitcmd::repo_root()?;
    let src_root = PathBuf::from(&repo_root);

    let total_files: u64 = selected
        .iter()
        .map(|&idx| count_files(&src_root.join(&candidates[idx])))
        .sum();

    let pb = ui::progress_bar(
        total_files.max(1),
        &format!(
            "Copying {} item{}…",
            selected.len(),
            if selected.len() == 1 { "" } else { "s" }
        ),
    );

    for idx in &selected {
        let rel = &candidates[*idx];
        let src = src_root.join(rel);
        let dst = dest.join(rel);
        copy_path(&src, &dst, &pb).with_context(|| {
            format!("Failed to copy '{}' to '{}'", src.display(), dst.display())
        })?;
    }

    pb.finish_and_clear();
    ui::print_success(&format!(
        "Copied {} item{}",
        selected.len(),
        if selected.len() == 1 { "" } else { "s" },
    ));

    Ok(())
}

/// Recursively count the number of regular files under `path`.
fn count_files(path: &Path) -> u64 {
    if path.is_file() {
        return 1;
    }
    if path.is_dir() {
        return fs::read_dir(path)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| count_files(&e.path()))
                    .sum()
            })
            .unwrap_or(0);
    }
    0
}

/// Copy `src` to `dst`, handling both regular files and directories.
fn copy_path(src: &Path, dst: &Path, pb: &indicatif::ProgressBar) -> Result<()> {
    if src.is_dir() {
        fs::create_dir_all(dst)
            .with_context(|| format!("Failed to create directory '{}'", dst.display()))?;
        for entry in fs::read_dir(src)
            .with_context(|| format!("Failed to read directory '{}'", src.display()))?
        {
            let entry =
                entry.with_context(|| format!("Failed to read entry in '{}'", src.display()))?;
            copy_path(&entry.path(), &dst.join(entry.file_name()), pb)?;
        }
    } else {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory '{}'", parent.display()))?;
        }
        let label = src
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        pb.set_message(label);
        fs::copy(src, dst).with_context(|| format!("Failed to copy file '{}'", src.display()))?;
        pb.inc(1);
    }
    Ok(())
}

/// Open an interactive subshell inside the workspace directory.
///
/// When `name` is `None`, a fuzzy picker is shown.
///
/// # Errors
///
/// Returns an error if:
/// - The workspace is not found in the store.
/// - The workspace directory no longer exists on disk.
/// - The shell process cannot be spawned.
pub fn switch(conn: &Connection, name: Option<&str>) -> Result<()> {
    let (_, workspaces) = load_repo_workspaces(conn)?;
    let worktrees = list_worktrees()?;
    let cwd = std::env::current_dir()?;

    let workspace: &WorkspaceRow = match name {
        Some(n) => workspaces
            .iter()
            .find(|w| w.name == n || w.name.to_lowercase().contains(&n.to_lowercase()))
            .with_context(|| {
                format!(
                    "Workspace '{}' not found. Run `{} workspace list` to see all workspaces.",
                    n,
                    crate::bin_name()
                )
            })?,

        None => {
            let candidates: Vec<&WorkspaceRow> = worktrees
                .iter()
                .filter(|wt| !wt.bare)
                .filter_map(|wt| workspaces.iter().find(|ws| Path::new(&ws.path) == wt.path))
                .collect();

            if candidates.is_empty() {
                ui::print_blank();
                println!(
                    "  {}",
                    "No workspaces found. Use `g workspace create <name>` to create one."
                        .bright_black()
                );
                ui::print_blank();
                return Ok(());
            }

            let name_width = candidates.iter().map(|ws| ws.name.len()).max().unwrap_or(0);
            let branch_width = candidates
                .iter()
                .map(|ws| ws.branch.len())
                .max()
                .unwrap_or(0);

            let items: Vec<String> = candidates
                .iter()
                .map(|ws| {
                    let marker = if cwd.starts_with(Path::new(&ws.path)) {
                        "\u{25c9}"
                    } else {
                        "\u{25ef}"
                    };
                    format!(
                        "{} {:<name_w$}  {:<branch_w$}  {}",
                        marker,
                        ws.name,
                        ws.branch,
                        ws.path,
                        name_w = name_width,
                        branch_w = branch_width,
                    )
                })
                .collect();

            let default_idx = candidates
                .iter()
                .position(|ws| cwd.starts_with(Path::new(&ws.path)))
                .unwrap_or(0);

            let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
                .with_prompt("Switch to workspace")
                .items(&items)
                .default(default_idx)
                .interact_opt()
                .context("Interactive workspace picker failed")?;

            match selection {
                None => {
                    ui::print_blank();
                    ui::print_info("Cancelled.");
                    ui::print_blank();
                    return Ok(());
                }
                Some(idx) => candidates[idx],
            }
        }
    };

    let wt_path = Path::new(&workspace.path);
    if !wt_path.exists() {
        bail!(
            "Workspace directory '{}' no longer exists. It may have been removed outside {}.\n\
             Run `{} workspace delete {}` to clean up.",
            wt_path.display(),
            crate::bin_name(),
            crate::bin_name(),
            workspace.name
        );
    }

    stats::record_workspace_event(conn, Some(workspace.id), Some(workspace.repo_id), "switch").ok();

    ui::print_blank();
    ui::print_info(&format!(
        "Opening shell in workspace {} \u{2192} {}",
        workspace.name.green().bold(),
        workspace.path.cyan().underline()
    ));
    let mut pairs: Vec<(&str, String)> =
        vec![("branch", workspace.branch.green().bold().to_string())];
    if let Some(desc) = &workspace.description {
        pairs.insert(0, ("desc", desc.bright_white().to_string()));
    }
    pairs.push((
        "hint",
        "Ctrl+D or `exit` to return".bright_black().to_string(),
    ));
    ui::print_key_value_pairs(&pairs);
    ui::print_blank();

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());

    let status = Command::new(&shell)
        .current_dir(wt_path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("Failed to spawn shell '{}'", shell))?;

    if !status.success() {
        if let Some(code) = status.code() {
            if code != 0 {
                ui::print_blank();
                ui::print_info(&format!("Shell exited with code {}", code));
            }
        }
    }

    ui::print_blank();
    ui::print_info("Returned to original directory.");
    ui::print_blank();

    Ok(())
}

/// Remove a workspace and its git worktree.
///
/// # Errors
///
/// Returns an error if:
/// - The workspace is not found.
/// - `git worktree remove` fails and the directory was not already missing.
pub fn delete(conn: &Connection, name: &str, force: bool) -> Result<()> {
    let (_, workspaces) = load_repo_workspaces(conn)?;

    let ws = workspaces
        .iter()
        .find(|w| w.name == name)
        .with_context(|| format!("Workspace '{}' not found.", name))?;

    let ws_id = ws.id;
    let wt_path = ws.path.clone();

    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(&wt_path);

    let force_note = if force { " (forced)" } else { "" };
    let result = gitcmd::git_mutate(
        &args,
        &format!("Remove worktree directory at {}{}", wt_path, force_note),
    );

    match result {
        Ok(_) => {}
        Err(e) => {
            if gitcmd::is_dry_run() {
                return Err(e);
            }
            let msg = format!("{}", e);
            if msg.contains("dirty") || msg.contains("untracked") {
                bail!(
                    "Worktree has uncommitted changes. Use `{} workspace delete {} --force` to remove anyway.",
                    crate::bin_name(),
                    name
                );
            }
            if !Path::new(&wt_path).exists() {
                ui::print_warning("Worktree directory already removed; cleaning up metadata.");
                gitcmd::git_output(&["worktree", "prune"]).ok();
            } else {
                return Err(e).context("Failed to remove worktree");
            }
        }
    }

    if !gitcmd::is_dry_run() {
        stats::record_workspace_event(conn, Some(ws_id), None, "delete").ok();
        ws_store::delete(conn, ws_id)?;
        ui::print_blank();
        ui::print_success(&format!("Deleted workspace '{}'", name.red()));
        ui::print_blank();
    } else {
        gitcmd::dry_run_action(
            "Remove workspace metadata",
            &format!("Delete workspace '{}' entry from g.db", name),
        );
    }

    Ok(())
}

/// Print status information about the current worktree.
///
/// # Errors
///
/// Returns an error if the store or git worktree listing cannot be read.
pub fn status(conn: &Connection) -> Result<()> {
    let (_, workspaces) = load_repo_workspaces(conn)?;
    let cwd = std::env::current_dir()?;
    let worktrees = list_worktrees()?;

    let current_wt = worktrees.iter().find(|wt| cwd.starts_with(&wt.path));

    ui::print_blank();

    if let Some(wt) = current_wt {
        let branch = wt.branch.as_deref().unwrap_or("(detached)");
        let meta = workspaces.iter().find(|ws| Path::new(&ws.path) == wt.path);

        if let Some(ws) = meta {
            let mut pairs: Vec<(&str, String)> = vec![
                (
                    "Workspace",
                    format!(
                        "{} {}",
                        ws.name.green().bold(),
                        format!("({})", ws.branch).bright_black()
                    ),
                ),
                ("Path", ws.path.cyan().underline().to_string()),
                ("Created", format_relative_time(ws.created_at)),
            ];
            if let Some(desc) = &ws.description {
                pairs.insert(1, ("Description", desc.to_string()));
            }
            ui::print_key_value_pairs(&pairs);
        } else {
            ui::print_key_value_pairs(&[(
                "Worktree",
                "(main repository)".green().bold().to_string(),
            )]);
        }

        ui::print_key_value_pairs(&[("Branch", branch.cyan().bold().to_string())]);

        let porcelain = gitcmd::git_output_lossy(&["status", "--porcelain"]);
        let changes: Vec<&str> = porcelain.lines().collect();
        if changes.is_empty() {
            ui::print_key_value_pairs(&[("Status", "clean".green().to_string())]);
        } else {
            let staged = changes
                .iter()
                .filter(|l| l.len() >= 2 && &l[0..1] != " " && &l[0..1] != "?")
                .count();
            let unstaged = changes
                .iter()
                .filter(|l| l.len() >= 2 && &l[1..2] != " " && &l[0..1] != "?")
                .count();
            let untracked = changes.iter().filter(|l| l.starts_with("??")).count();
            let status_val = format!(
                "{} change{}{}{}",
                changes.len().to_string().yellow(),
                if changes.len() == 1 { "" } else { "s" },
                if staged > 0 {
                    format!(" ({} staged)", staged).green().to_string()
                } else {
                    String::new()
                },
                if untracked > 0 {
                    format!(" ({} untracked)", untracked)
                        .bright_black()
                        .to_string()
                } else if unstaged > 0 {
                    format!(" ({} unstaged)", unstaged).yellow().to_string()
                } else {
                    String::new()
                },
            );
            ui::print_key_value_pairs(&[("Status", status_val)]);
        }
    } else {
        ui::print_info("Not inside any known worktree.");
    }

    ui::print_blank();
    Ok(())
}

/// Rename a workspace by moving its directory and repairing git worktree tracking.
///
/// # Errors
///
/// Returns an error if:
/// - The workspace is not found.
/// - The new directory already exists on disk.
/// - The old directory no longer exists.
/// - The directory move or `git worktree repair` fails.
pub fn rename(conn: &Connection, old: &str, new: &str) -> Result<()> {
    let (_, workspaces) = load_repo_workspaces(conn)?;

    let ws = workspaces
        .iter()
        .find(|w| w.name == old)
        .with_context(|| format!("Workspace '{}' not found.", old))?;

    let ws_id = ws.id;
    let old_path = PathBuf::from(&ws.path);
    let new_path = worktree_path_for(conn, new)?;

    if new_path.exists() {
        bail!(
            "Directory '{}' already exists. Choose a different name.",
            new_path.display()
        );
    }

    if !old_path.exists() {
        bail!(
            "Workspace directory '{}' no longer exists. Clean up with `{} workspace delete {}`.",
            old_path.display(),
            crate::bin_name(),
            old
        );
    }

    if !gitcmd::is_dry_run() {
        let pb = ui::spinner(&format!("Moving worktree {} → {}", old, new));

        fs::rename(&old_path, &new_path).with_context(|| {
            format!(
                "Failed to move '{}' to '{}'",
                old_path.display(),
                new_path.display()
            )
        })?;

        gitcmd::git_output(&["worktree", "repair"])
            .context("Failed to repair worktree tracking after move")?;

        ws_store::update_name_and_path(conn, ws_id, new, &new_path.to_string_lossy())?;
        stats::record_workspace_event(conn, Some(ws_id), None, "rename").ok();

        ui::spinner_success(
            pb,
            &format!("Renamed workspace '{}' \u{2192} '{}'", old, new.green()),
        );
        println!(
            "  {} {}",
            "path:".bright_black(),
            new_path.display().to_string().cyan().underline()
        );
        ui::print_blank();
    } else {
        gitcmd::dry_run_action(
            &format!("mv {} {}", old_path.display(), new_path.display()),
            &format!(
                "Move worktree directory from '{}' to '{}'",
                old_path.display(),
                new_path.display()
            ),
        );
        gitcmd::git_mutate(
            &["worktree", "repair"],
            "Repair git worktree tracking after the directory move",
        )?;
        gitcmd::dry_run_action(
            "Update workspace metadata",
            &format!("Rename workspace '{}' → '{}' in g.db", old, new),
        );
    }

    Ok(())
}
