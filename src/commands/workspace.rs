//! Workspace (git worktree) management.
//!
//! ## Tutorial overview
//!
//! A "workspace" here is a git worktree plus a small metadata record stored in
//! `~/.config/g/workspaces.toml`.  Git itself is the source of truth for
//! which worktrees exist on disk; this module only adds extra UI metadata
//! (human-friendly name, description, creation timestamp).
//!
//! The public functions in this module are called from `main.rs` when the user
//! runs `g workspace <subcommand>`.
//!
//! ## Rust concepts used here
//!
//! - `serde` derives (`Serialize`, `Deserialize`) to read/write TOML metadata.
//! - `Option<T>` for optional fields like `description`.
//! - `Path` (borrowed) vs `PathBuf` (owned) for filesystem paths.
//! - Iterators and pattern matching to parse git porcelain output line-by-line.
//! - `chrono::DateTime<Utc>` for time-zone-aware creation timestamps.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::commands::git as gitcmd;
use crate::config;
use crate::ui;

// ─── Data structures ──────────────────────────────────────────────────────────

/// User-visible workspace metadata stored in `workspaces.toml`.
///
/// Git worktrees are identified by their filesystem path.  The fields here are
/// purely for display purposes; they do not affect git's own worktree tracking.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Workspace {
    /// Human-friendly workspace name used in `g workspace switch <name>`.
    pub name: String,
    /// Optional one-line description shown in `g workspace list`.
    ///
    /// `Option<String>` means "may be absent" — Rust has no `null`.
    pub description: Option<String>,
    /// Absolute filesystem path to the worktree directory.
    pub path: String,
    /// Branch associated with the worktree at creation time.
    pub branch: String,
    /// UTC timestamp used to display "created X days ago" in list view.
    pub created_at: DateTime<Utc>,
}

/// Persistent store: the top-level TOML table in `workspaces.toml`.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct WorkspaceStore {
    /// All known workspace metadata entries in insertion order.
    pub workspaces: Vec<Workspace>,
}

/// Live worktree info parsed from `git worktree list --porcelain`.
///
/// This struct is crate-internal; callers work with the higher-level
/// [`Workspace`] type from the store.
struct WorktreeInfo {
    /// Absolute filesystem path to the worktree directory.
    path: PathBuf,
    /// Full object name of the HEAD commit in this worktree.
    #[allow(dead_code)]
    head: String,
    /// Branch name (short ref name), or `None` in detached-HEAD state.
    branch: Option<String>,
    /// `true` when this entry represents a bare repository.
    bare: bool,
}

// ─── Persistence ─────────────────────────────────────────────────────────────

/// Read the workspace store from `workspaces.toml`, or return an empty default.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be read or parsed.
fn load_store() -> Result<WorkspaceStore> {
    let path = config::workspaces_path()?;
    if !path.exists() {
        return Ok(WorkspaceStore::default());
    }
    let raw = fs::read_to_string(&path).context("Failed to read workspaces file")?;
    toml::from_str(&raw).context("Failed to parse workspaces file")
}

/// Serialise `store` and write it to `workspaces.toml`.
///
/// # Errors
///
/// Returns an error if serialisation or the file write fails.
fn save_store(store: &WorkspaceStore) -> Result<()> {
    let path = config::workspaces_path()?;
    let raw = toml::to_string_pretty(store).context("Failed to serialize workspaces")?;
    fs::write(&path, raw).context("Failed to save workspaces file")
}

// ─── Git worktree helpers ─────────────────────────────────────────────────────

/// Query git for all worktree entries and parse the `--porcelain` format.
///
/// The porcelain format emits blocks of key-value pairs, each block starting
/// with `worktree <path>`.  We parse these line-by-line, accumulating state
/// for the current block.
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
            // Flush the previous block before starting a new one.
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
            // Strip "refs/heads/" so we store just the short name.
            current_branch = Some(b.strip_prefix("refs/heads/").unwrap_or(b).to_string());
        } else if line == "bare" {
            is_bare = true;
        }
    }

    // Flush the last block.
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

/// Compute the default sibling-directory path for a new worktree named `name`.
///
/// For example, with a repo at `/home/user/myapp` and separator `"--"`, a
/// workspace named `feature-x` would be placed at `/home/user/myapp--feature-x`.
///
/// # Errors
///
/// Returns an error if the repo root or config cannot be determined.
fn worktree_path_for(name: &str) -> Result<PathBuf> {
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

/// Render a human-readable "time ago" string for `dt` (e.g. `"3 days ago"`).
fn format_relative_time(dt: DateTime<Utc>) -> String {
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

/// List all git worktrees with their optional `g` metadata.
///
/// Combines live worktree data from git with display metadata from the store.
/// The current working directory is highlighted with a `◉` marker.
///
/// # Errors
///
/// Returns an error if the store or git worktree listing cannot be read.
pub fn list() -> Result<()> {
    let store = load_store()?;
    let worktrees = list_worktrees()?;
    let cwd = std::env::current_dir()?;

    if worktrees.is_empty() {
        println!();
        println!("  {}", "No worktrees found.".bright_black());
        println!();
        return Ok(());
    }

    println!();
    let mut table = ui::Table::new(vec!["", "Name", "Branch", "Path", "Created"]);

    for wt in &worktrees {
        if wt.bare {
            continue;
        }

        let branch_display = wt.branch.as_deref().unwrap_or("(detached)");
        let is_current = cwd.starts_with(&wt.path);

        // Look up optional g metadata for this worktree by path.
        let meta = store
            .workspaces
            .iter()
            .find(|ws| Path::new(&ws.path) == wt.path);

        let (name_display, created_display) = if let Some(ws) = meta {
            let name = if is_current {
                ws.name.green().bold().to_string()
            } else {
                ws.name.white().to_string()
            };
            let label = if let Some(desc) = &ws.description {
                if !desc.is_empty() {
                    format!("{}  {}", name, desc.bright_black())
                } else {
                    name
                }
            } else {
                name
            };
            (label, format_relative_time(ws.created_at))
        } else {
            // Main worktree or untracked worktree.
            let name = if is_current {
                "(main)".green().bold().to_string()
            } else {
                "(main)".bright_black().to_string()
            };
            (name, "—".bright_black().to_string())
        };

        let marker = if is_current {
            "◉".green().bold().to_string()
        } else {
            "◯".bright_black().to_string()
        };

        let path_display = wt.path.display().to_string().bright_black().to_string();

        table.add_row(vec![
            marker,
            name_display,
            ui::color_branch(branch_display),
            path_display,
            created_display,
        ]);
    }

    table.print();
    println!();

    if store.workspaces.is_empty() {
        println!(
            "  {} {}",
            "tip:".bright_black(),
            format!(
                "{} workspace create <name>  to create a worktree workspace",
                crate::bin_name()
            )
            .bright_black()
        );
        println!();
    }

    Ok(())
}

/// Create a new worktree for `branch` (or a new branch named after the workspace)
/// and save its metadata to the store.
///
/// The worktree is placed in a sibling directory computed by [`worktree_path_for`].
///
/// # Errors
///
/// Returns an error if:
/// - A workspace with the same name already exists.
/// - The computed directory already exists on disk.
/// - `git worktree add` fails.
/// - The store cannot be saved.
pub fn create(name: &str, branch: Option<&str>, description: Option<&str>) -> Result<()> {
    let mut store = load_store()?;

    if store.workspaces.iter().any(|w| w.name == name) {
        bail!("Workspace '{}' already exists. Use a different name.", name);
    }

    let wt_path = worktree_path_for(name)?;
    if wt_path.exists() {
        bail!(
            "Directory '{}' already exists. Choose a different workspace name.",
            wt_path.display()
        );
    }

    let branch_name = branch.unwrap_or(name);
    let wt_path_str = wt_path.to_string_lossy().to_string();

    // Choose between checking out an existing branch or creating a new one.
    let branch_exists = gitcmd::git_output(&["rev-parse", "--verify", branch_name]).is_ok();

    let result = if branch_exists {
        gitcmd::git_mutate(
            &["worktree", "add", &wt_path_str, branch_name],
            &format!(
                "Create worktree for existing branch '{}' at {}",
                branch_name, wt_path_str
            ),
        )
    } else {
        gitcmd::git_mutate(
            &["worktree", "add", "-b", branch_name, &wt_path_str],
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
        store.workspaces.push(Workspace {
            name: name.to_string(),
            description: description.map(str::to_string),
            path: wt_path_str.clone(),
            branch: branch_name.to_string(),
            created_at: Utc::now(),
        });
        save_store(&store)?;

        println!();
        ui::print_success(&format!(
            "Created workspace {} on branch {}",
            name.green().bold(),
            branch_name.cyan()
        ));
        println!(
            "     {} {}",
            "path:".bright_black(),
            wt_path_str.cyan().underline()
        );
        println!();
        println!(
            "  {} {}",
            "tip:".bright_black(),
            format!(
                "{} workspace switch {}  to open a shell there",
                crate::bin_name(),
                name
            )
            .bright_black()
        );
        println!();
    } else {
        gitcmd::dry_run_action(
            "Save workspace metadata",
            &format!(
                "Register workspace '{}' on branch '{}' in workspaces.toml",
                name, branch_name
            ),
        );
    }

    Ok(())
}

/// Open an interactive subshell inside the workspace directory.
///
/// Spawns `$SHELL` (or `/bin/sh` as a fallback) with its working directory
/// set to the workspace path.  The user exits the shell to return.
///
/// # Errors
///
/// Returns an error if:
/// - The workspace is not found in the store.
/// - The workspace directory no longer exists on disk.
/// - The shell process cannot be spawned.
pub fn switch(name: &str) -> Result<()> {
    let store = load_store()?;

    let workspace = store
        .workspaces
        .iter()
        .find(|w| w.name == name || w.name.contains(name))
        .with_context(|| {
            format!(
                "Workspace '{}' not found. Run `{} workspace list` to see all workspaces.",
                name,
                crate::bin_name()
            )
        })?;

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

    println!();
    ui::print_info(&format!(
        "Opening shell in workspace {} → {}",
        workspace.name.green().bold(),
        workspace.path.cyan().underline()
    ));
    if let Some(desc) = &workspace.description {
        println!("     {} {}", "desc:".bright_black(), desc.bright_white());
    }
    println!(
        "     {} {}",
        "branch:".bright_black(),
        workspace.branch.green().bold()
    );
    println!(
        "     {}",
        "Exit the shell (Ctrl+D or `exit`) to return.".bright_black()
    );
    println!();

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
                println!();
                ui::print_info(&format!("Shell exited with code {}", code));
            }
        }
    }

    println!();
    ui::print_info("Returned to original directory.");
    println!();

    Ok(())
}

/// Remove a workspace and its git worktree, optionally forcing removal of dirty trees.
///
/// If the worktree directory was already removed manually, `git worktree prune`
/// is run to clean up git's internal tracking before removing the metadata entry.
///
/// # Errors
///
/// Returns an error if:
/// - The workspace is not found in the store.
/// - `git worktree remove` fails and the directory was not already missing.
/// - The store cannot be saved.
pub fn delete(name: &str, force: bool) -> Result<()> {
    let mut store = load_store()?;

    let idx = store
        .workspaces
        .iter()
        .position(|w| w.name == name)
        .with_context(|| format!("Workspace '{}' not found.", name))?;

    let workspace = &store.workspaces[idx];
    let wt_path = workspace.path.clone();

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
        store.workspaces.remove(idx);
        save_store(&store)?;

        println!();
        ui::print_success(&format!("Deleted workspace '{}'", name.red()));
        println!();
    } else {
        gitcmd::dry_run_action(
            "Remove workspace metadata",
            &format!("Delete workspace '{}' entry from workspaces.toml", name),
        );
    }
    Ok(())
}

/// Print status information about the current worktree.
///
/// Shows the workspace name, description, branch, creation time, and a summary
/// of staged/unstaged/untracked changes.  If the cwd is not inside any known
/// worktree, a brief message is shown instead.
///
/// # Errors
///
/// Returns an error if the store or git worktree listing cannot be read.
pub fn status() -> Result<()> {
    let store = load_store()?;
    let cwd = std::env::current_dir()?;
    let worktrees = list_worktrees()?;

    let current_wt = worktrees.iter().find(|wt| cwd.starts_with(&wt.path));

    println!();

    if let Some(wt) = current_wt {
        let branch = wt.branch.as_deref().unwrap_or("(detached)");
        let meta = store
            .workspaces
            .iter()
            .find(|ws| Path::new(&ws.path) == wt.path);

        if let Some(ws) = meta {
            println!(
                "  {} {} {}",
                "Workspace:".bright_black(),
                ws.name.green().bold(),
                format!("({})", ws.branch).bright_black()
            );
            if let Some(desc) = &ws.description {
                println!("  {} {}", "Description:".bright_black(), desc);
            }
            println!(
                "  {} {}",
                "Path:".bright_black(),
                ws.path.cyan().underline()
            );
            println!(
                "  {} {}",
                "Created:".bright_black(),
                format_relative_time(ws.created_at)
            );
        } else {
            println!(
                "  {} {}",
                "Worktree:".bright_black(),
                "(main repository)".green().bold()
            );
        }

        println!("  {} {}", "Branch:".bright_black(), branch.cyan().bold());

        let porcelain = gitcmd::git_output_lossy(&["status", "--porcelain"]);
        let changes: Vec<&str> = porcelain.lines().collect();
        if changes.is_empty() {
            println!("  {} {}", "Status:".bright_black(), "clean".green());
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
            println!(
                "  {} {} change{}{}{}",
                "Status:".bright_black(),
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
        }
    } else {
        println!("  {}", "Not inside any known worktree.".bright_black());
    }

    println!();
    Ok(())
}

/// Rename a workspace by moving its directory and repairing git worktree tracking.
///
/// After the directory move, `git worktree repair` updates git's internal
/// `.git/worktrees/<name>/gitdir` symlink to the new path.
///
/// # Errors
///
/// Returns an error if:
/// - The workspace is not found in the store.
/// - The new directory name already exists on disk.
/// - The old directory no longer exists.
/// - The directory cannot be moved (`fs::rename`).
/// - `git worktree repair` fails.
/// - The store cannot be saved.
pub fn rename(old: &str, new: &str) -> Result<()> {
    let mut store = load_store()?;

    let ws = store
        .workspaces
        .iter_mut()
        .find(|w| w.name == old)
        .with_context(|| format!("Workspace '{}' not found.", old))?;

    let old_path = PathBuf::from(&ws.path);
    let new_path = worktree_path_for(new)?;

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

        pb.finish_and_clear();

        ws.name = new.to_string();
        ws.path = new_path.to_string_lossy().to_string();
        save_store(&store)?;

        println!();
        ui::print_success(&format!("Renamed workspace '{}' → '{}'", old, new.green()));
        println!(
            "     {} {}",
            "path:".bright_black(),
            new_path.display().to_string().cyan().underline()
        );
        println!();
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
            &format!("Rename workspace '{}' → '{}' in workspaces.toml", old, new),
        );
    }

    Ok(())
}
