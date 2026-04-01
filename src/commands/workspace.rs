//! Workspace (git worktree) management.
//!
//! ## Overview
//!
//! A "workspace" is a git worktree plus a small metadata record stored in
//! `~/.config/g/workspaces.toml`.  Git is the source of truth for which
//! worktrees exist on disk; this module adds UI metadata (friendly name,
//! description, creation timestamp).
//!
//! The store is keyed by repository root path so metadata from multiple
//! repositories coexists in the same file without colliding.  For repos using
//! the container layout (set by `init` or `clone --workspace`), the key is the
//! container directory — which equals the original repo root before `init` —
//! so it remains stable after the directory reorganisation.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, MultiSelect};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::commands::git as gitcmd;
use crate::config;
use crate::ui;

// ─── Data structures ──────────────────────────────────────────────────────────

/// User-visible workspace metadata stored in `workspaces.toml`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Workspace {
    /// Human-friendly name used in `g workspace switch <name>`.
    pub name: String,
    /// Optional one-line description shown in `g workspace list`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Absolute filesystem path to the worktree directory.
    pub path: String,
    /// Branch associated with the worktree at creation time.
    pub branch: String,
    /// UTC timestamp used to display "created X days ago".
    pub created_at: DateTime<Utc>,
}

/// Per-repository metadata stored under a single key in [`WorkspaceStore`].
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct RepoStore {
    /// All known workspace metadata entries in insertion order.
    #[serde(default)]
    pub workspaces: Vec<Workspace>,
    /// Set by `init` or `clone --workspace`.
    ///
    /// When present, new worktrees are placed as direct children of this
    /// directory (`<container_root>/<name>`) instead of as siblings with
    /// the separator convention (`<parent>/<repo_name>--<name>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_root: Option<String>,
}

/// Top-level TOML file: all repo stores keyed by their root path.
///
/// Example on disk:
///
/// ```toml
/// [repos."/home/user/myapp"]
/// container_root = "/home/user/myapp"
///
/// [[repos."/home/user/myapp".workspaces]]
/// name = "feature-auth"
/// branch = "feat/auth"
/// path = "/home/user/myapp/feature-auth"
/// created_at = "2026-01-01T00:00:00Z"
///
/// [repos."/home/user/other-project"]
///
/// [[repos."/home/user/other-project".workspaces]]
/// name = "hotfix"
/// branch = "fix/crash"
/// path = "/home/user/other-project--hotfix"
/// created_at = "2026-01-02T00:00:00Z"
/// ```
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct WorkspaceStore {
    /// Per-repo metadata, keyed by repository root path.
    #[serde(default)]
    pub repos: HashMap<String, RepoStore>,
}

/// Live worktree info parsed from `git worktree list --porcelain`.
struct WorktreeInfo {
    path: PathBuf,
    head: String,
    branch: Option<String>,
    bare: bool,
}

// ─── Persistence ─────────────────────────────────────────────────────────────

/// Read the full workspace store from disk, or return an empty default.
///
/// If the file is present but cannot be parsed (corruption), a warning is
/// printed and an empty store is returned so that all commands continue to
/// work.  The reconcile step in [`load_repo_store`] will then re-populate the
/// store from live git data.
fn load_store() -> Result<WorkspaceStore> {
    let path = config::workspaces_path()?;
    if !path.exists() {
        return Ok(WorkspaceStore::default());
    }
    let raw = fs::read_to_string(&path).context("Failed to read workspaces file")?;
    match toml::from_str::<WorkspaceStore>(&raw) {
        Ok(store) => Ok(store),
        Err(e) => {
            // Corrupted TOML — warn and fall back to empty so the tool keeps
            // working.  The caller will reconcile from git.
            ui::print_warning(&format!(
                "workspaces.toml could not be parsed ({e}). Starting fresh — your git worktrees are safe."
            ));
            Ok(WorkspaceStore::default())
        }
    }
}

/// Serialise and write the full store to disk using an atomic rename so that
/// a crash mid-write never leaves a partial (corrupt) file.
///
/// # Errors
///
/// Returns an error if serialisation or the file write fails.
fn save_store(store: &WorkspaceStore) -> Result<()> {
    let path = config::workspaces_path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).context("Failed to create config directory")?;
    }
    let raw = toml::to_string_pretty(store).context("Failed to serialize workspaces")?;

    // Write to a sibling temp file, then atomically rename over the target.
    // This prevents a partial write from corrupting the live store.
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, &raw).context("Failed to write workspace store temp file")?;
    fs::rename(&tmp, &path).context("Failed to atomically replace workspaces file")
}

/// Determine the store key for the current repository.
///
/// The key is stable across the container-layout reorganisation performed by
/// `init`: the container root equals the original repo root, so data written
/// before and after `init` lives under the same key.
///
/// Algorithm:
/// 1. Get the current repo root via `git rev-parse --show-toplevel`.
/// 2. Scan existing store entries: if any `container_root` is a prefix of the
///    current repo root (i.e. we are inside a known container), return that
///    entry's key.
/// 3. Otherwise return the repo root itself.
///
/// # Errors
///
/// Returns an error if `git rev-parse` fails (not inside a git repo).
fn current_repo_key(store: &WorkspaceStore) -> Result<String> {
    let repo_root = gitcmd::repo_root()?;

    for (key, repo) in &store.repos {
        if let Some(ref croot) = repo.container_root {
            if repo_root.starts_with(croot.as_str()) {
                return Ok(key.clone());
            }
        }
    }

    Ok(repo_root)
}

/// Load the full store, resolve the current repo key, reconcile with live git
/// worktrees, and return `(full_store, key, repo_store)`.
///
/// The reconcile step silently registers any git worktrees that exist on disk
/// but are absent from the store.  This self-heals after crashes, failed
/// writes, or manual `git worktree add` calls.  If any new entries were added,
/// the store is persisted immediately.
///
/// # Errors
///
/// Propagates errors from [`load_store`] or [`current_repo_key`].
fn load_repo_store() -> Result<(WorkspaceStore, String, RepoStore)> {
    let mut store = load_store()?;
    let key = current_repo_key(&store)?;

    // Reconcile: add any live worktrees that are missing from the store.
    let dirty = reconcile_store_with_git(&mut store, &key);
    if dirty {
        // Best-effort — a save failure here is non-fatal; the in-memory store
        // is still correct for this invocation.
        let _ = save_store(&store);
    }

    let repo = store.repos.get(&key).cloned().unwrap_or_default();
    Ok((store, key, repo))
}

/// Compare live git worktrees against the stored entries for `key` and add
/// any missing entries with metadata derived from git.
///
/// Returns `true` if any entries were added (store is now dirty).
fn reconcile_store_with_git(store: &mut WorkspaceStore, key: &str) -> bool {
    // If we're not inside a git repo, skip silently.
    let worktrees = match list_worktrees() {
        Ok(wt) => wt,
        Err(_) => return false,
    };

    let repo = store.repos.entry(key.to_string()).or_default();
    let mut added = false;

    for wt in &worktrees {
        if wt.bare {
            continue;
        }
        let path_str = wt.path.to_string_lossy().to_string();

        // Skip if already registered.
        if repo.workspaces.iter().any(|w| w.path == path_str) {
            continue;
        }

        // Skip the main worktree path when a container_root is set — the
        // main workspace is registered explicitly by `init`/`clone`.
        // For other layouts every worktree we find is worth registering.
        let branch = wt.branch.as_deref().unwrap_or("unknown").to_string();

        // Derive a human-readable name from the directory name.
        let name = wt
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| branch.clone());

        repo.workspaces.push(Workspace {
            name,
            description: None,
            path: path_str,
            branch,
            created_at: Utc::now(),
        });
        added = true;
    }

    added
}

/// Insert the updated [`RepoStore`] back into the full store under `key` and
/// write it to disk.
///
/// # Errors
///
/// Propagates errors from [`save_store`].
fn save_repo_store(mut store: WorkspaceStore, key: &str, repo: RepoStore) -> Result<()> {
    store.repos.insert(key.to_string(), repo);
    save_store(&store)
}

// ─── Git worktree helpers ─────────────────────────────────────────────────────

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
/// Uses the container layout when `container_root` is set for the current
/// repo; otherwise falls back to the sibling layout.
///
/// # Errors
///
/// Returns an error if the repo root or config cannot be determined.
fn worktree_path_for(name: &str) -> Result<PathBuf> {
    let (_, _, repo) = load_repo_store()?;

    if let Some(container) = &repo.container_root {
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

/// Render a human-readable "time ago" string for `dt`.
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
/// The store is saved before `git worktree repair` runs so that a repair
/// failure does not leave the filesystem and store out of sync.
///
/// # Errors
///
/// Returns an error if:
/// - The current directory is not inside a git repository.
/// - The repo is already in container layout for this repository.
/// - The user cancels the confirmation prompt.
/// - Any filesystem or git operation fails.
pub fn init() -> Result<()> {
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
    // Using load_repo_store() resolves the key correctly even when running
    // from inside a container child (e.g. vcli/main after a previous init).
    let (store, key, repo) = load_repo_store()?;
    if repo.container_root.is_some() {
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
    println!();
    println!("  {} workspace init", "g".green().bold());
    println!();
    println!(
        "  This will reorganise '{}' into a container layout:",
        repo_root.display().to_string().cyan()
    );
    println!();
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
    println!();
    println!(
        "  After this, new workspaces will be created inside '{}'.",
        container_dir.display().to_string().cyan()
    );
    println!();

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
        println!();
        println!("  {}", "Cancelled.".bright_black());
        println!();
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
        // Attempt rollback.
        if temp_path.exists() && !repo_root.exists() {
            let _ = fs::rename(&temp_path, &repo_root);
        }
        return Err(e).context("Init failed — attempted to roll back to original state");
    }

    let container_root_str = container_dir.to_string_lossy().to_string();
    let inner_dir_str = inner_dir.to_string_lossy().to_string();

    // Save the store BEFORE the repair step so that a repair failure does not
    // leave the filesystem and store out of sync.
    let mut updated_repo = repo;
    updated_repo.container_root = Some(container_root_str.clone());
    // Register the main workspace if it isn't already there.
    if !updated_repo
        .workspaces
        .iter()
        .any(|w| w.path == inner_dir_str)
    {
        updated_repo.workspaces.push(Workspace {
            name: branch_name.clone(),
            description: None,
            path: inner_dir_str.clone(),
            branch: branch_name.clone(),
            created_at: Utc::now(),
        });
    }
    save_repo_store(store, &key, updated_repo)?;

    // Step 4: repair git worktree tracking after the directory move.
    // Use `git -C <inner_dir>` so git finds both the .git dir and the work
    // tree.  `--git-dir` alone (without `--work-tree`) fails when the process
    // cwd was changed to the parent above.
    gitcmd::git_output(&["-C", &inner_dir_str, "worktree", "repair"])
        .context("Failed to repair git worktree tracking after move")?;
    // Prune is best-effort — stale entries are harmless, just untidy.
    let _ = gitcmd::git_output(&["-C", &inner_dir_str, "worktree", "prune"]);

    println!();
    ui::print_success(&format!(
        "Repository reorganised into container layout at {}",
        container_dir.display().to_string().cyan()
    ));
    println!(
        "     {} {}",
        "main workspace:".bright_black(),
        inner_dir_str.cyan().underline()
    );
    println!();
    println!(
        "  {} cd {}",
        "next:".bright_black(),
        inner_dir.display().to_string().green()
    );
    println!();

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
pub fn clone_with_workspace(args: &[String]) -> Result<()> {
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
        // The key for a freshly cloned repo is the container dir itself.
        let mut full_store = load_store()?;
        let repo = full_store
            .repos
            .entry(container_root_str.clone())
            .or_default();
        repo.container_root = Some(container_root_str.clone());
        if !repo.workspaces.iter().any(|w| w.path == inner_dir_str) {
            repo.workspaces.push(Workspace {
                name: default_branch.clone(),
                description: None,
                path: inner_dir_str.clone(),
                branch: default_branch.clone(),
                created_at: Utc::now(),
            });
        }
        save_store(&full_store)?;

        println!();
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
        println!();
        println!(
            "  {} cd {}",
            "next:".bright_black(),
            inner_dir.display().to_string().green()
        );
        println!();
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

/// List all git worktrees with their optional `g` metadata.
///
/// Combines live worktree data from git with metadata from the current repo's
/// store.  The current working directory is highlighted with a `◉` marker.
///
/// # Errors
///
/// Returns an error if the store or git worktree listing cannot be read.
pub fn list() -> Result<()> {
    let (_, _, repo) = load_repo_store()?;
    let worktrees = list_worktrees()?;
    let cwd = std::env::current_dir()?;

    if worktrees.is_empty() {
        println!();
        println!("  {}", "No worktrees found.".bright_black());
        println!();
        return Ok(());
    }

    println!();
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

        let meta = repo
            .workspaces
            .iter()
            .find(|ws| Path::new(&ws.path) == wt.path);

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
            (name, "—".bright_black().to_string())
        };

        let marker = if is_current {
            "◉".green().bold().to_string()
        } else {
            "◯".bright_black().to_string()
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
    println!();

    if repo.workspaces.is_empty() {
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

/// Create a new worktree and save its metadata to the current repo's store.
///
/// Branch resolution order:
/// 1. Branch exists locally → check out directly.
/// 2. Branch exists on `origin` → create a local tracking branch.
/// 3. Neither → create a new branch (optionally from `start_point`).
///
/// When `copy` is `true`, an interactive [`MultiSelect`] lets the user pick
/// untracked and gitignored files to copy into the new worktree.
///
/// # Errors
///
/// Returns an error if:
/// - A workspace with the same name already exists in the current repo.
/// - The computed directory already exists on disk.
/// - `git worktree add` fails.
/// - The file-copy step fails (when `copy` is `true`).
/// - The store cannot be saved.
pub fn create(
    name: &str,
    branch: Option<&str>,
    start_point: Option<&str>,
    description: Option<&str>,
    copy: bool,
) -> Result<()> {
    let (store, key, mut repo) = load_repo_store()?;

    if repo.workspaces.iter().any(|w| w.name == name) {
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
        // Register the workspace in the store BEFORE the copy step so that a
        // copy failure (or cancellation) does not orphan the worktree from the
        // store.  The worktree already exists on disk at this point.
        repo.workspaces.push(Workspace {
            name: name.to_string(),
            description: description.map(str::to_string),
            path: wt_path_str.clone(),
            branch: branch_name.to_string(),
            created_at: Utc::now(),
        });
        save_repo_store(store, &key, repo)?;
    } else {
        gitcmd::dry_run_action(
            "Save workspace metadata",
            &format!(
                "Register workspace '{}' on branch '{}' in workspaces.toml",
                name, branch_name
            ),
        );
    }

    // Copy untracked/gitignored files — runs after the store is saved so a
    // failure here does not orphan the workspace entry.
    if copy && !gitcmd::is_dry_run() {
        copy_untracked_files(&wt_path)?;
    } else if copy {
        gitcmd::dry_run_action(
            "Copy untracked files",
            "Show interactive picker and copy selected files to new worktree",
        );
    }

    if !gitcmd::is_dry_run() {
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
    }

    Ok(())
}

/// Present an interactive checklist of untracked and gitignored files and copy
/// the user's selection into `dest`.
///
/// # Errors
///
/// Returns an error if the interactive prompt fails or a file copy fails.
fn copy_untracked_files(dest: &Path) -> Result<()> {
    // Untracked files (not gitignored) — individual file paths.
    let untracked_out =
        gitcmd::git_output(&["ls-files", "--others", "--exclude-standard"]).unwrap_or_default();

    // Gitignored entries — use --directory so large trees like node_modules
    // appear as a single selectable item rather than thousands of files.
    let ignored_out = gitcmd::git_output(&[
        "ls-files",
        "--others",
        "-i",
        "--exclude-standard",
        "--directory",
    ])
    .unwrap_or_default();

    // Merge both lists.  Strip trailing slashes so paths are uniform and
    // can be used directly with Path::new().
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

    println!();
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

    // Count total files across all selected items so the progress bar has an
    // accurate total before any copying starts.
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
///
/// Returns `1` for a regular file, `0` for anything that cannot be read, and
/// the recursive sum for a directory.
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
///
/// Increments `pb` by one for each regular file copied and updates the
/// progress message with the file's name.
///
/// # Errors
///
/// Returns an error if any filesystem operation fails.
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
        // Show the file name (not the full path) to keep the bar readable.
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
/// When `name` is `None`, a [`FuzzySelect`] picker is shown so the user can
/// search and choose.  When `name` is `Some`, the workspace is fuzzy-matched
/// from the current repo's store.
///
/// # Errors
///
/// Returns an error if:
/// - The workspace is not found in the store.
/// - The workspace directory no longer exists on disk.
/// - The shell process cannot be spawned.
pub fn switch(name: Option<&str>) -> Result<()> {
    let (_, _, repo) = load_repo_store()?;
    let worktrees = list_worktrees()?;
    let cwd = std::env::current_dir()?;

    let workspace: &Workspace = match name {
        Some(n) => repo
            .workspaces
            .iter()
            // Exact match first, then case-insensitive substring fallback.
            .find(|w| w.name == n || w.name.to_lowercase().contains(&n.to_lowercase()))
            .with_context(|| {
                format!(
                    "Workspace '{}' not found. Run `{} workspace list` to see all workspaces.",
                    n,
                    crate::bin_name()
                )
            })?,

        None => {
            let candidates: Vec<&Workspace> = worktrees
                .iter()
                .filter(|wt| !wt.bare)
                .filter_map(|wt| {
                    repo.workspaces
                        .iter()
                        .find(|ws| Path::new(&ws.path) == wt.path)
                })
                .collect();

            if candidates.is_empty() {
                println!();
                println!(
                    "  {}",
                    "No workspaces found. Use `g workspace create <name>` to create one."
                        .bright_black()
                );
                println!();
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
                        "◉"
                    } else {
                        "◯"
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
                    println!();
                    println!("  {}", "Cancelled.".bright_black());
                    println!();
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

/// Remove a workspace and its git worktree.
///
/// If the directory was already removed manually, `git worktree prune` cleans
/// up git's internal tracking before removing the metadata entry.
///
/// # Errors
///
/// Returns an error if:
/// - The workspace is not found in the current repo's store.
/// - `git worktree remove` fails and the directory was not already missing.
/// - The store cannot be saved.
pub fn delete(name: &str, force: bool) -> Result<()> {
    let (store, key, mut repo) = load_repo_store()?;

    let idx = repo
        .workspaces
        .iter()
        .position(|w| w.name == name)
        .with_context(|| format!("Workspace '{}' not found.", name))?;

    let wt_path = repo.workspaces[idx].path.clone();

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
        repo.workspaces.remove(idx);
        save_repo_store(store, &key, repo)?;
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
/// # Errors
///
/// Returns an error if the store or git worktree listing cannot be read.
pub fn status() -> Result<()> {
    let (_, _, repo) = load_repo_store()?;
    let cwd = std::env::current_dir()?;
    let worktrees = list_worktrees()?;

    let current_wt = worktrees.iter().find(|wt| cwd.starts_with(&wt.path));

    println!();

    if let Some(wt) = current_wt {
        let branch = wt.branch.as_deref().unwrap_or("(detached)");
        let meta = repo
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
/// # Errors
///
/// Returns an error if:
/// - The workspace is not found in the current repo's store.
/// - The new directory already exists on disk.
/// - The old directory no longer exists.
/// - The directory move or `git worktree repair` fails.
/// - The store cannot be saved.
pub fn rename(old: &str, new: &str) -> Result<()> {
    let (store, key, mut repo) = load_repo_store()?;

    let ws = repo
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
        save_repo_store(store, &key, repo)?;

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
