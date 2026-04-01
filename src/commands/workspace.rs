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
use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, MultiSelect};
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
    /// Set by `init` or `clone --workspace`.
    ///
    /// When present, new worktrees are placed as direct children of this
    /// directory (e.g. `<container_root>/feature-x`) instead of as sibling
    /// directories with the separator convention
    /// (`<parent>/<repo_name>--feature-x`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_root: Option<String>,
}

/// Live worktree info parsed from `git worktree list --porcelain`.
///
/// This struct is crate-internal; callers work with the higher-level
/// [`Workspace`] type from the store.
struct WorktreeInfo {
    /// Absolute filesystem path to the worktree directory.
    path: PathBuf,
    /// Full object name of the HEAD commit in this worktree.
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

/// Compute the filesystem path for a new worktree named `name`.
///
/// Two conventions are supported:
///
/// 1. **Container layout** (set by `init` / `clone --workspace`): when
///    `store.container_root` is set, the new worktree is placed directly
///    inside that directory — e.g. `<container_root>/feature-x`.
///
/// 2. **Sibling layout** (default): the new worktree is placed next to the
///    repo root, separated by the configured separator — e.g.
///    `/home/user/myapp--feature-x`.
///
/// # Errors
///
/// Returns an error if the repo root or config cannot be determined.
fn worktree_path_for(name: &str) -> Result<PathBuf> {
    let store = load_store()?;

    if let Some(container) = &store.container_root {
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

/// Reorganise an existing, non-worktree repo into the container layout.
///
/// Given a repo at `/path/to/repo` on branch `main`, the operation is:
///
/// ```text
/// mv  /path/to/repo         /path/to/repo--ws-tmp
/// mkdir                     /path/to/repo           (container)
/// mv  /path/to/repo--ws-tmp /path/to/repo/main
/// ```
///
/// After this, `g workspace create` will place new worktrees as siblings of
/// `main` inside the container directory.
///
/// The command asks for confirmation before making any changes and rolls back
/// on failure.
///
/// # Errors
///
/// Returns an error if:
/// - The current directory is not inside a git repository.
/// - The repo is already in the container layout (double-init guard).
/// - The user cancels the confirmation prompt.
/// - Any filesystem or git operation fails.
pub fn init() -> Result<()> {
    let repo_root_str =
        gitcmd::repo_root().context("Not inside a git repository. Run `git init` first.")?;
    let repo_root = PathBuf::from(&repo_root_str);

    // Guard: already init'd if the store has a container_root set.
    let mut store = load_store()?;
    if store.container_root.is_some() {
        bail!(
            "This repository is already using the container workspace layout.\n\
             Run `{} workspace list` to see your workspaces.",
            crate::bin_name()
        );
    }

    // Guard: if the repo root's parent contains a directory with the same
    // name we'd create, something unexpected is already there.
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

    let container_dir = repo_root.clone(); // will be recreated as the container
    let inner_dir = container_dir.join(&branch_name);

    // A unique temp name in the parent, not inside the repo (we're about to move it).
    let temp_name = format!("{}--ws-tmp", repo_name);
    let temp_path = parent.join(&temp_name);

    if temp_path.exists() {
        bail!(
            "Temporary path '{}' already exists. Please remove it and try again.",
            temp_path.display()
        );
    }

    // Show the plan and ask for confirmation.
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
            "git worktree repair",
            "Repair git worktree tracking after directory move",
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

    // Step out of the repo directory before we move it.
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

    // Step 2 + 3: create container, move temp → inner.
    let result = (|| -> Result<()> {
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

    if let Err(e) = result {
        // Attempt rollback: move temp back to the original repo path.
        if temp_path.exists() && !repo_root.exists() {
            let _ = fs::rename(&temp_path, &repo_root);
        }
        return Err(e).context("Init failed — attempted to roll back to original state");
    }

    // Step 4: repair git worktree tracking after the directory move.
    gitcmd::git_output(&[
        "--git-dir",
        &format!("{}/.git", inner_dir.display()),
        "worktree",
        "repair",
    ])
    .context("Failed to repair git worktree tracking after move")?;

    // Persist the container root and register the main workspace.
    let container_root_str = container_dir.to_string_lossy().to_string();
    let inner_dir_str = inner_dir.to_string_lossy().to_string();
    store.container_root = Some(container_root_str);
    store.workspaces.push(Workspace {
        name: branch_name.clone(),
        description: None,
        path: inner_dir_str.clone(),
        branch: branch_name.clone(),
        created_at: Utc::now(),
    });
    save_store(&store)?;

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
/// Equivalent to running `g clone <url> [dest] --workspace`:
///
/// 1. Queries the remote for its default branch via `git ls-remote`.
/// 2. Creates a container directory named after the repo.
/// 3. Clones into `<container>/<default_branch>` so the primary worktree is
///    immediately ready for `g workspace create`.
///
/// Any extra flags in `args` (e.g. `--depth 1`, `-q`) are forwarded to
/// `git clone` verbatim.
///
/// # Errors
///
/// Returns an error if:
/// - No URL is found in `args`.
/// - The container directory already exists.
/// - `git ls-remote` or `git clone` fails.
/// - The store cannot be saved.
pub fn clone_with_workspace(args: &[String]) -> Result<()> {
    // Find the URL — the first arg that isn't a flag.
    let url = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .with_context(|| "No URL found. Usage: g clone <url> [dest] --workspace")?;

    // Derive the container directory name from the URL: strip trailing `/`,
    // take the last path segment, and remove a `.git` suffix.
    let repo_name = url
        .trim_end_matches('/')
        .split('/')
        .next_back()
        .unwrap_or("repo")
        .trim_end_matches(".git")
        .to_string();

    // Allow the user to supply an explicit destination as the second non-flag
    // positional arg (mirrors `git clone <url> <dest>`).
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

    // Determine the remote's default branch before cloning.
    let default_branch = detect_remote_default_branch(url).unwrap_or_else(|_| "main".to_string());

    let inner_dir = container_dir.join(&default_branch);
    let inner_dir_str = inner_dir.to_string_lossy().to_string();

    fs::create_dir_all(&container_dir).with_context(|| {
        format!(
            "Failed to create container directory '{}'",
            container_dir.display()
        )
    })?;

    // Build git clone args: forward all original flags, replace dest with inner_dir.
    let mut clone_args: Vec<&str> = vec!["clone"];
    // Forward flags only (not positional args — we replace those).
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
        // Clean up the container dir on failure.
        let _ = fs::remove_dir_all(&container_dir);
        return Err(e).context("Clone failed");
    }

    if !gitcmd::is_dry_run() {
        // Load (or create) the store and set the container root.
        let mut store = load_store()?;
        let container_root_str = container_dir.to_string_lossy().to_string();
        store.container_root = Some(container_root_str.clone());
        store.workspaces.push(Workspace {
            name: default_branch.clone(),
            description: None,
            path: inner_dir_str.clone(),
            branch: default_branch.clone(),
            created_at: Utc::now(),
        });
        save_store(&store)?;

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

/// Query the remote for its default branch using `git ls-remote --symref`.
///
/// Parses a line of the form `ref: refs/heads/<branch>\tHEAD` to extract
/// the branch name.  Falls back to `Err` when the remote does not advertise
/// a HEAD symref (e.g. some GitHub mirrors).
///
/// # Errors
///
/// Returns an error if `git ls-remote` fails or the output cannot be parsed.
fn detect_remote_default_branch(url: &str) -> Result<String> {
    let output = gitcmd::git_output(&["ls-remote", "--symref", url, "HEAD"])
        .context("Failed to query remote default branch")?;

    for line in output.lines() {
        // Expected format: `ref: refs/heads/main\tHEAD`
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
    let mut table = ui::Table::new(vec!["", "Name", "Branch", "Path", "HEAD", "Created"]);

    for wt in &worktrees {
        if wt.bare {
            continue;
        }

        let branch_display = wt.branch.as_deref().unwrap_or("(detached)");
        let is_current = cwd.starts_with(&wt.path);

        // Truncate the full SHA to a 7-char short hash for display.
        let head_display = if wt.head.len() >= 7 {
            wt.head[..7].bright_black().to_string()
        } else {
            wt.head.bright_black().to_string()
        };

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
            head_display,
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
/// The worktree is placed in a directory computed by [`worktree_path_for`]:
/// inside `container_root` when the repo was set up with `init`/`clone
/// --workspace`, or as a sibling directory otherwise.
///
/// When `copy` is `true`, an interactive [`MultiSelect`] checklist of
/// untracked and gitignored files is shown; the user picks which ones to copy
/// into the new worktree.
///
/// # Errors
///
/// Returns an error if:
/// - A workspace with the same name already exists.
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

    // Three-stage branch resolution:
    //  1. Branch exists locally              → checkout directly
    //  2. Branch exists on origin (remote)   → create local tracking branch
    //  3. Neither                            → create a fresh branch (optional start_point)
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
        // Build args for a new branch, optionally from a start point.
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

    // Copy untracked / gitignored files from the current worktree into the new one.
    if copy && !gitcmd::is_dry_run() {
        copy_untracked_files(&wt_path)?;
    }

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
        if copy {
            gitcmd::dry_run_action(
                "Copy untracked files",
                "Show interactive picker and copy selected files to new worktree",
            );
        }
    }

    Ok(())
}

/// Present an interactive checklist of untracked and gitignored files from the
/// current working tree and copy the user's selection into `dest`.
///
/// Silently returns `Ok(())` when there are no files to copy or the user
/// selects nothing.
///
/// # Errors
///
/// Returns an error if the interactive prompt fails or a file copy fails.
fn copy_untracked_files(dest: &Path) -> Result<()> {
    // Collect untracked (not ignored) files.
    let untracked_out =
        gitcmd::git_output(&["ls-files", "--others", "--exclude-standard"]).unwrap_or_default();
    // Collect gitignored files (e.g. .env, node_modules).
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
        .map(str::to_string)
        .collect();

    // Deduplicate while preserving order.
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

    for idx in selected {
        let rel = &candidates[idx];
        let src = src_root.join(rel);
        let dst = dest.join(rel);

        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory '{}'", parent.display()))?;
        }

        fs::copy(&src, &dst).with_context(|| {
            format!("Failed to copy '{}' to '{}'", src.display(), dst.display())
        })?;
    }

    Ok(())
}

/// Open an interactive subshell inside the workspace directory.
///
/// When `name` is `Some`, the workspace is fuzzy-matched by name from the
/// store and the shell is opened immediately.  When `name` is `None`, a
/// `FuzzySelect` picker is shown so the user can search and choose.
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
pub fn switch(name: Option<&str>) -> Result<()> {
    let store = load_store()?;
    let worktrees = list_worktrees()?;
    let cwd = std::env::current_dir()?;

    // Resolve which workspace to open — either by the supplied name or via an
    // interactive fuzzy picker when no name was given.
    let workspace: &Workspace = match name {
        // ── Named switch ──────────────────────────────────────────────────────
        Some(n) => store
            .workspaces
            .iter()
            .find(|w| w.name == n || w.name.contains(n))
            .with_context(|| {
                format!(
                    "Workspace '{}' not found. Run `{} workspace list` to see all workspaces.",
                    n,
                    crate::bin_name()
                )
            })?,

        // ── Interactive picker ────────────────────────────────────────────────
        None => {
            // Build the candidate list from live worktrees joined with store
            // metadata.  Bare worktrees are excluded.
            let candidates: Vec<&Workspace> = worktrees
                .iter()
                .filter(|wt| !wt.bare)
                .filter_map(|wt| {
                    store
                        .workspaces
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

            // Build display strings with consistent column widths so the fuzzy
            // matcher has clean, readable lines to search against.
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

            // Pre-select the current workspace if we're inside one.
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
