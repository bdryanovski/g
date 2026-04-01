//! Stacked PR workflow management.
//!
//! ## Tutorial overview
//!
//! This module implements the "Stacked Pull Requests" workflow.  It tracks
//! ordered lists of branches (called *stacks*) in `~/.config/g/stacks.toml`.
//!
//! Key operations:
//! - `new`    — start a stack rooted at the current branch.
//! - `add`    — append a new branch on top of the current stack position.
//! - `squash` — collapse the current branch to a single commit, then restack above.
//! - `fold`   — merge the current branch into its parent, then restack above.
//! - `sync`   — rebase the whole chain so each branch sits cleanly on the one below.
//! - `pr`     — create or update GitHub PRs so each PR targets the branch below.
//!
//! ## Rust concepts used here
//!
//! - `HashMap` keyed by repo root for isolating stacks per repository.
//! - `Vec<StackBranch>` for the ordered, linear nature of a stack.
//! - `chrono` for UTC timestamps.
//! - Multi-step git operations (rebase chains, soft-reset squash, merges).
//! - Silenceable network calls so a missing GitHub token never blocks the UI.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

use crate::commands::git as gitcmd;
use crate::config;
use crate::github;
use crate::ui;

// ─── Data structures ──────────────────────────────────────────────────────────

/// A named, ordered sequence of branches that form a stacked-PR chain.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Stack {
    /// Human-readable stack name (e.g. `"auth-refactor"`).
    pub name: String,
    /// The branch at the bottom of the stack — the eventual merge target.
    pub root: String,
    /// Ordered list of branches from bottom (index 0) to top.
    pub branches: Vec<StackBranch>,
    /// When the stack was first created.
    pub created_at: DateTime<Utc>,
    /// When the stack was last modified (branch added, squashed, etc.).
    pub updated_at: DateTime<Utc>,
}

/// Metadata for a single branch within a stack.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StackBranch {
    /// Short branch name (no `refs/heads/` prefix).
    pub name: String,
    /// GitHub PR number if a PR has been created for this branch.
    pub pr_number: Option<u64>,
    /// GitHub PR web URL if a PR has been created for this branch.
    pub pr_url: Option<String>,
    /// Optional one-line description shown in stack views.
    pub description: Option<String>,
}

/// All stacks for a single repository.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct RepoStacks {
    /// Stacks in insertion order.
    pub stacks: Vec<Stack>,
}

/// Root of the `stacks.toml` file, keyed by absolute repository root path.
///
/// Keying by repo root means one config file can hold stacks for many
/// repositories without naming conflicts.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct StackStore {
    /// Maps absolute repo root paths to their stacks.
    #[serde(default)]
    pub repositories: HashMap<String, RepoStacks>,
}

// ─── Persistence ─────────────────────────────────────────────────────────────

/// Load the stack store from disk, or return an empty default.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be read or parsed.
fn load_store() -> Result<StackStore> {
    let path = config::stacks_path()?;
    if !path.exists() {
        return Ok(StackStore::default());
    }
    let raw = fs::read_to_string(&path).context("Failed to read stacks file")?;
    toml::from_str(&raw).context("Failed to parse stacks file")
}

/// Serialise `store` and write it to `stacks.toml`.
///
/// # Errors
///
/// Returns an error if serialisation or the file write fails.
fn save_store(store: &StackStore) -> Result<()> {
    let path = config::stacks_path()?;
    let raw = toml::to_string_pretty(store).context("Failed to serialize stacks")?;
    fs::write(&path, raw).context("Failed to save stacks file")
}

// ─── Store helpers ────────────────────────────────────────────────────────────

/// Return the current repository identifier (its absolute root path).
///
/// # Errors
///
/// Returns an error if `git rev-parse --show-toplevel` fails.
fn repo_id() -> Result<String> {
    gitcmd::repo_root()
}

/// Return a shared reference to the stack list for `repo`, or `None`.
fn repo_stacks<'a>(store: &'a StackStore, repo: &str) -> Option<&'a Vec<Stack>> {
    store.repositories.get(repo).map(|r| &r.stacks)
}

/// Return an exclusive reference to the stack list for `repo`, creating it if absent.
fn repo_stacks_mut<'a>(store: &'a mut StackStore, repo: &str) -> &'a mut Vec<Stack> {
    &mut store
        .repositories
        .entry(repo.to_string())
        .or_default()
        .stacks
}

/// Find the stack that contains the current branch.
///
/// # Errors
///
/// Returns an error if there are no stacks or the current branch is not in any stack.
fn current_stack(store: &StackStore) -> Result<&Stack> {
    let repo = repo_id()?;
    let branch = gitcmd::current_branch()?;

    let stacks = repo_stacks(store, &repo).with_context(|| {
        format!(
            "No stacks in this repository. Use `{} stack new <name>` to create one.",
            crate::bin_name()
        )
    })?;

    stacks
        .iter()
        .find(|s| s.branches.iter().any(|b| b.name == branch))
        .with_context(|| {
            format!(
                "Branch '{}' is not part of any stack. Use `{} stack new <name>` to create one.",
                branch,
                crate::bin_name()
            )
        })
}

/// Find the stack that contains `branch` within a slice of stacks, or `None`.
fn find_stack_for_branch<'a>(stacks: &'a [Stack], branch: &str) -> Option<&'a Stack> {
    stacks
        .iter()
        .find(|s| s.branches.iter().any(|b| b.name == branch))
}

/// Retrieve the GitHub token from `GITHUB_TOKEN` env var or the config file.
///
/// # Errors
///
/// Returns an error if neither source provides a non-empty token.
fn get_github_token(cfg: &config::Config) -> Result<String> {
    if let Ok(t) = std::env::var("GITHUB_TOKEN") {
        return Ok(t);
    }
    cfg.github
        .token
        .clone()
        .filter(|t| !t.is_empty())
        .with_context(|| {
            "GitHub token not found. Set GITHUB_TOKEN env var or add `token` to [github] in config."
                .to_string()
        })
}

/// Open `url` in the default browser for the current OS.
fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(url).spawn()?;
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open").arg(url).spawn()?;
    #[cfg(target_os = "windows")]
    std::process::Command::new("cmd")
        .args(["/C", "start", url])
        .spawn()?;
    Ok(())
}

// ─── Shared UI helpers ────────────────────────────────────────────────────────

/// Print the standard "rebase conflict" instructions to the terminal.
///
/// This single function replaces three near-identical blocks that were previously
/// copy-pasted in `squash`, `sync`, and `restack_branches_from`.  Centralising
/// the message means fixing a typo only needs to happen in one place.
fn print_conflict_instructions(branch: &str) {
    let cmd = crate::bin_name();
    ui::print_warning(&format!(
        "Conflict in {}: resolve manually, then run `{} stack sync` again",
        branch.yellow(),
        cmd
    ));
    println!();
    println!("  {} After resolving conflicts:", "→".cyan());
    println!("    {} git add <files>", "1.".bright_black());
    println!("    {} git rebase --continue", "2.".bright_black());
    println!(
        "    {} {cmd} stack sync  (to straighten any remaining branches)",
        "3.".bright_black()
    );
    println!();
}

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Create a new stack rooted at the current branch.
///
/// The current branch becomes the first entry in the stack's branch list and
/// also the `root` (the eventual merge target for the entire chain).
///
/// # Errors
///
/// Returns an error if a stack with the same name already exists or the store
/// cannot be saved.
pub fn new_stack(name: &str) -> Result<()> {
    let mut store = load_store()?;
    let repo = repo_id()?;
    let branch = gitcmd::current_branch()?;

    let stacks = repo_stacks_mut(&mut store, &repo);

    if stacks.iter().any(|s| s.name == name) {
        bail!("Stack '{}' already exists in this repository.", name);
    }

    if !gitcmd::is_dry_run() {
        let now = Utc::now();
        stacks.push(Stack {
            name: name.to_string(),
            root: branch.clone(),
            branches: vec![StackBranch {
                name: branch.clone(),
                pr_number: None,
                pr_url: None,
                description: None,
            }],
            created_at: now,
            updated_at: now,
        });
        save_store(&store)?;

        println!();
        ui::print_success(&format!(
            "Created stack {} rooted at {}",
            name.cyan().bold(),
            branch.green().bold()
        ));
        println!();
    } else {
        gitcmd::dry_run_action(
            "Create stack metadata",
            &format!(
                "Register stack '{}' rooted at branch '{}' in stacks.toml",
                name, branch
            ),
        );
    }
    Ok(())
}

/// Create a new branch directly above the current stack position and add it to the stack.
///
/// The new branch is checked out immediately after creation.
///
/// # Errors
///
/// Returns an error if the current branch is not in a stack, `git checkout -b`
/// fails, or the store cannot be saved.
pub fn add_branch(branch_name: &str) -> Result<()> {
    let mut store = load_store()?;
    let repo = repo_id()?;
    let current_branch = gitcmd::current_branch()?;

    // Capture position before we mutate `store`.
    let (stack_name, current_pos) = {
        let stack = current_stack(&store)?;
        let pos = stack
            .branches
            .iter()
            .position(|b| b.name == current_branch)
            .with_context(|| format!("Branch '{}' not found in stack", current_branch))?;
        (stack.name.clone(), pos)
    };

    gitcmd::git_mutate(
        &["checkout", "-b", branch_name],
        &format!(
            "Create new branch '{}' from current HEAD and switch to it",
            branch_name
        ),
    )
    .with_context(|| format!("Failed to create branch '{}'", branch_name))?;

    if !gitcmd::is_dry_run() {
        let stacks = repo_stacks_mut(&mut store, &repo);
        let stack = stacks
            .iter_mut()
            .find(|s| s.name == stack_name)
            .with_context(|| format!("Stack '{}' disappeared after branch creation", stack_name))?;

        stack.branches.insert(
            current_pos + 1,
            StackBranch {
                name: branch_name.to_string(),
                pr_number: None,
                pr_url: None,
                description: None,
            },
        );
        stack.updated_at = Utc::now();
        save_store(&store)?;

        println!();
        ui::print_success(&format!(
            "Created branch {} and added to stack",
            branch_name.green().bold()
        ));
        println!();
    } else {
        gitcmd::dry_run_action(
            "Update stack metadata",
            &format!(
                "Insert '{}' into stack '{}' at position {}",
                branch_name,
                stack_name,
                current_pos + 1
            ),
        );
    }
    Ok(())
}

/// List all stacks in the current repository.
///
/// # Errors
///
/// Returns an error if the store cannot be read.
pub fn list() -> Result<()> {
    let store = load_store()?;
    let repo = repo_id()?;
    let current_branch = gitcmd::current_branch().unwrap_or_default();

    let stacks = match repo_stacks(&store, &repo) {
        Some(s) if !s.is_empty() => s,
        _ => {
            println!();
            println!("  {}", "No stacks yet.".bright_black());
            ui::print_tip(&format!(
                "{} stack new <name>  to create a stack from the current branch",
                crate::bin_name()
            ));
            println!();
            return Ok(());
        }
    };

    for stack in stacks {
        println!();
        println!(
            "  {} {}  {}",
            "Stack:".bright_black(),
            stack.name.cyan().bold(),
            format!("(root: {})", stack.root).bright_black()
        );
        println!();

        let last = stack.branches.len().saturating_sub(1);
        for (i, branch) in stack.branches.iter().enumerate() {
            let is_current = branch.name == current_branch;
            let connector = if i == last {
                "  └──"
            } else {
                "  ├──"
            };
            let marker = ui::branch_marker(is_current);
            let name_colored = ui::branch_name_colored(&branch.name, is_current);

            print!("{} {} {}", connector.bright_black(), marker, name_colored);

            if let Some(pr_url) = &branch.pr_url {
                let pr_num = branch
                    .pr_number
                    .map(|n| format!(" #{}", n))
                    .unwrap_or_default();
                print!("  {}{}", "PR".bright_black(), pr_num.cyan());
                print!("  {}", pr_url.bright_black().underline());
            }

            if is_current {
                print!("  {}", "← you are here".bright_black());
            }
            println!();

            if i < last {
                println!("  {}   {}", "│".bright_black(), "│".bright_black());
            }
        }
    }
    println!();
    Ok(())
}

/// Alias for [`list`] — shows the current stack tree.
///
/// # Errors
///
/// Propagates any error from [`list`].
pub fn view() -> Result<()> {
    list()
}

/// Show the current stack with per-branch commit details and live PR status.
///
/// Attempts to fetch open PRs from GitHub.  Silently skips the network call if
/// no token is configured or the call fails, so this command always works offline.
///
/// # Errors
///
/// Returns an error if the store cannot be read.
pub fn details() -> Result<()> {
    let store = load_store()?;
    let stack = current_stack(&store)?.clone();
    let current_branch = gitcmd::current_branch().unwrap_or_default();

    let open_prs = fetch_open_prs_for_details();

    println!();
    println!(
        "  {} {}  {}",
        "Stack:".bright_black(),
        stack.name.cyan().bold(),
        format!("(root: {})", stack.root).bright_black()
    );
    println!();

    let last_branch = stack.branches.len().saturating_sub(1);

    for (i, branch) in stack.branches.iter().enumerate() {
        let is_current = branch.name == current_branch;
        let connector = if i == last_branch {
            "└──"
        } else {
            "├──"
        };
        let pipe = if i == last_branch { " " } else { "│" };

        let marker = ui::branch_marker(is_current);
        let name_colored = ui::branch_name_colored(&branch.name, is_current);

        print!("  {} {} {}", connector.bright_black(), marker, name_colored);
        if is_current {
            print!("  {}", "(current)".green().dimmed());
        }
        println!();

        let branch_time = gitcmd::git_output_lossy(&["log", "-1", "--format=%ar", &branch.name]);
        if !branch_time.is_empty() {
            println!(
                "  {}     {}",
                pipe.bright_black(),
                branch_time.trim().bright_black()
            );
        }

        // Show the PR — live from the API if available, otherwise from the store.
        let live_pr = open_prs.as_ref().and_then(|prs| prs.get(&branch.name));
        if let Some(pr) = live_pr {
            println!(
                "  {}     {} {}  {}",
                pipe.bright_black(),
                "PR".bright_black(),
                format!("#{}", pr.number).cyan(),
                pr.html_url.bright_black().underline()
            );
        } else if let Some(pr_url) = &branch.pr_url {
            let pr_num = branch
                .pr_number
                .map(|n| format!("#{}", n))
                .unwrap_or_default();
            println!(
                "  {}     {} {}  {}",
                pipe.bright_black(),
                "PR".bright_black(),
                pr_num.cyan(),
                pr_url.bright_black().underline()
            );
        }

        // Show commits between this branch and the one below it.
        let base = if i == 0 {
            &stack.root
        } else {
            &stack.branches[i - 1].name
        };

        if i > 0 || branch.name != stack.root {
            let range = format!("{}..{}", base, branch.name);
            let commits = gitcmd::git_output_lossy(&[
                "log",
                "--format=%h%x1f%s%x1f%an%x1f%ar",
                "--reverse",
                &range,
            ]);

            if !commits.is_empty() {
                println!("  {}", pipe.bright_black());
                for commit_line in commits.lines() {
                    let parts: Vec<&str> = commit_line.split('\x1f').collect();
                    if parts.len() >= 4 {
                        println!(
                            "  {}     {} - {}  {}",
                            pipe.bright_black(),
                            parts[0].yellow().dimmed(),
                            parts[1].bright_black(),
                            format!("({}, {})", parts[2], parts[3])
                                .bright_black()
                                .dimmed()
                        );
                    } else if let Some((hash, subject)) = commit_line.split_once(' ') {
                        println!(
                            "  {}     {} - {}",
                            pipe.bright_black(),
                            hash.yellow().dimmed(),
                            subject.bright_black()
                        );
                    }
                }
            } else {
                println!(
                    "  {}     {}",
                    pipe.bright_black(),
                    "(no commits)".bright_black()
                );
            }
        }

        if i < last_branch {
            println!("  {}   {}", "│".bright_black(), "│".bright_black());
        }
    }

    println!();
    Ok(())
}

/// Try to fetch all open PRs from GitHub for display in [`details`].
///
/// Returns `None` silently when no token is configured or the API call fails.
fn fetch_open_prs_for_details() -> Option<HashMap<String, github::PrInfo>> {
    let cfg = config::load().ok()?;
    let token = get_github_token(&cfg).ok()?;
    let (owner, repo_name) = github::detect_repo().ok()?;
    github::list_open_prs(&token, &cfg.github.api_base, &owner, &repo_name).ok()
}

/// Switch to the top branch of the named stack.
///
/// # Errors
///
/// Returns an error if no stacks exist, the named stack is not found, the stack
/// is empty, or `git checkout` fails.
pub fn switch_stack(name: &str) -> Result<()> {
    let store = load_store()?;
    let repo = repo_id()?;

    let stacks =
        repo_stacks(&store, &repo).with_context(|| "No stacks in this repository.".to_string())?;

    let stack = stacks
        .iter()
        .find(|s| s.name == name || s.name.contains(name))
        .with_context(|| {
            format!(
                "Stack '{}' not found. Run `{} stack list` to see all stacks.",
                name,
                crate::bin_name()
            )
        })?;

    let top_branch = stack
        .branches
        .last()
        .with_context(|| format!("Stack '{}' has no branches.", name))?;

    let current = gitcmd::current_branch().unwrap_or_default();
    if current == top_branch.name && !gitcmd::is_dry_run() {
        println!();
        ui::print_info(&format!(
            "Already on stack {} (branch {})",
            stack.name.cyan().bold(),
            top_branch.name.green().bold()
        ));
        println!();
        return Ok(());
    }

    gitcmd::git_mutate(
        &["checkout", &top_branch.name],
        &format!(
            "Switch to top branch '{}' of stack '{}'",
            top_branch.name, stack.name
        ),
    )
    .with_context(|| format!("Failed to checkout branch '{}'", top_branch.name))?;

    if !gitcmd::is_dry_run() {
        println!();
        ui::print_success(&format!(
            "Switched to stack {} → branch {}",
            stack.name.cyan().bold(),
            top_branch.name.green().bold()
        ));
        println!();
    }
    Ok(())
}

/// Merge the current branch into the branch immediately below it in the stack.
///
/// The current branch is deleted after a successful `--no-ff` merge and is
/// removed from the stack metadata.
///
/// # Errors
///
/// Returns an error if the current branch is not in a stack, is already at the
/// bottom, or any git operation fails.
pub fn absorb() -> Result<()> {
    let mut store = load_store()?;
    let repo = repo_id()?;
    let current_branch = gitcmd::current_branch()?;

    // Snapshot name and position before any mutable borrow.
    let (stack_name, pos) = {
        let stacks = repo_stacks(&store, &repo)
            .with_context(|| "No stacks in this repository.".to_string())?;
        let stack = find_stack_for_branch(stacks, &current_branch)
            .with_context(|| format!("Branch '{}' is not part of any stack.", current_branch))?;
        let pos = stack
            .branches
            .iter()
            .position(|b| b.name == current_branch)
            .with_context(|| format!("Branch '{}' not found in stack", current_branch))?;
        (stack.name.clone(), pos)
    };

    if pos == 0 {
        println!();
        ui::print_warning("This is the bottom branch of the stack — nothing below to absorb into.");
        println!();
        return Ok(());
    }

    let target_branch = {
        let stacks = repo_stacks(&store, &repo)
            .with_context(|| "Stack disappeared unexpectedly".to_string())?;
        let stack = stacks
            .iter()
            .find(|s| s.name == stack_name)
            .with_context(|| format!("Stack '{}' not found", stack_name))?;
        stack.branches[pos - 1].name.clone()
    };
    let absorbed_branch = current_branch.clone();

    gitcmd::git_mutate(
        &["checkout", &target_branch],
        &format!("Switch to target branch '{}' for merge", target_branch),
    )
    .with_context(|| format!("Failed to checkout '{}'", target_branch))?;

    gitcmd::git_mutate(
        &["merge", "--no-ff", &absorbed_branch],
        &format!(
            "Merge '{}' into '{}' with a merge commit (--no-ff)",
            absorbed_branch, target_branch
        ),
    )
    .with_context(|| {
        if !gitcmd::is_dry_run() {
            let _ = gitcmd::git_output(&["merge", "--abort"]);
            let _ = gitcmd::git_output(&["checkout", &absorbed_branch]);
        }
        "Failed to merge branches".to_string()
    })?;

    gitcmd::git_mutate(
        &["branch", "-d", &absorbed_branch],
        &format!(
            "Delete the absorbed branch '{}' (safe delete, only if fully merged)",
            absorbed_branch
        ),
    )?;

    if !gitcmd::is_dry_run() {
        let remaining_count = {
            let stacks = repo_stacks_mut(&mut store, &repo);
            let stack = stacks
                .iter_mut()
                .find(|s| s.name == stack_name)
                .with_context(|| format!("Stack '{}' not found after absorb", stack_name))?;
            stack.branches.remove(pos);
            stack.updated_at = Utc::now();
            stack.branches.len()
        };
        save_store(&store)?;

        println!();
        ui::print_success(&format!(
            "Absorbed {} into {}",
            absorbed_branch.green().bold(),
            target_branch.cyan().bold()
        ));
        println!(
            "     {} Stack now has {} branch{}",
            "".bright_black(),
            remaining_count.to_string().yellow(),
            if remaining_count == 1 { "" } else { "es" }
        );
        println!();
    } else {
        gitcmd::dry_run_action(
            "Update stack metadata",
            &format!(
                "Remove '{}' from stack '{}' in stacks.toml",
                absorbed_branch, stack_name
            ),
        );
    }
    Ok(())
}

/// Rebase each branch from index `start` upward onto the one below it.
///
/// Returns `Ok(true)` when all rebases completed without conflicts.
/// Returns `Ok(false)` when a conflict was found and `no_interactive` is `false`
/// (the user resolves it manually and re-runs `g stack sync`).
///
/// # Errors
///
/// Returns an error when `no_interactive` is `true` and a conflict is found
/// (the rebase is automatically aborted).
fn restack_branches_from(stack: &Stack, start: usize, no_interactive: bool) -> Result<bool> {
    if start == 0 || stack.branches.len() <= start {
        return Ok(true);
    }
    for i in start..stack.branches.len() {
        let base = stack.branches[i - 1].name.clone();
        let branch = stack.branches[i].name.clone();

        gitcmd::git_mutate(
            &["checkout", &branch],
            &format!("Switch to branch '{}' to restack", branch),
        )
        .with_context(|| format!("Failed to checkout '{}'", branch))?;

        let result = gitcmd::git_mutate(
            &["rebase", &base],
            &format!(
                "Rebase '{}' onto '{}' so upstack branches follow the new spine",
                branch, base
            ),
        );

        match result {
            Ok(_) => {
                if !gitcmd::is_dry_run() {
                    ui::print_success(&format!(
                        "{} rebased onto {}",
                        branch.green().bold(),
                        base.cyan()
                    ));
                }
            }
            Err(e) => {
                if no_interactive {
                    let _ = gitcmd::git_output(&["rebase", "--abort"]);
                    bail!(
                        "Conflict rebasing '{}' onto '{}': {}\nRun without --no-interactive to resolve manually.",
                        branch, base, e
                    );
                } else {
                    print_conflict_instructions(&branch);
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

/// Merge the current branch into its parent (or vice-versa with `--keep`) and
/// drop the now-redundant branch from the stack.
///
/// - Without `--keep`: merge `current` into `parent`, delete `current`, keep `parent`.
/// - With `--keep`:    merge `parent` into `current`, delete `parent`, keep `current`.
///
/// Branches above the fold point are rebased onto the surviving branch.
///
/// # Errors
///
/// Returns an error if the working tree is dirty, the current branch is at the
/// bottom of the stack, or any git operation fails.
pub fn fold(keep: bool, no_interactive: bool) -> Result<()> {
    let mut store = load_store()?;
    let repo = repo_id()?;
    let current = gitcmd::current_branch()?;

    let (stack_name, stack_snapshot, pos, parent, child) = {
        let stacks = repo_stacks(&store, &repo)
            .with_context(|| "No stacks in this repository.".to_string())?;
        let stack = find_stack_for_branch(stacks, &current)
            .with_context(|| format!("Branch '{}' is not part of any stack.", current))?;
        let pos = stack
            .branches
            .iter()
            .position(|b| b.name == current)
            .with_context(|| format!("Branch '{}' not found in stack", current))?;
        if pos == 0 {
            bail!(
                "Cannot fold: '{}' is the bottom branch of the stack (no parent below it).",
                current
            );
        }
        let parent = stack.branches[pos - 1].name.clone();
        let child = stack.branches[pos].name.clone();
        (stack.name.clone(), stack.clone(), pos, parent, child)
    };

    gitcmd::require_clean_tree("folding")?;

    // Compute the post-fold branch list and where to start restacking.
    let (result_branch, restack_start, new_branches, new_root) = if !keep {
        let mut nb = stack_snapshot.branches.clone();
        nb.remove(pos);
        (parent.clone(), pos, nb, stack_snapshot.root.clone())
    } else {
        let mut nb = stack_snapshot.branches.clone();
        nb.remove(pos - 1);
        let nr = if stack_snapshot.root == parent {
            child.clone()
        } else {
            stack_snapshot.root.clone()
        };
        (child.clone(), pos, nb, nr)
    };

    let restack_stack = Stack {
        name: stack_snapshot.name.clone(),
        root: new_root.clone(),
        branches: new_branches.clone(),
        created_at: stack_snapshot.created_at,
        updated_at: stack_snapshot.updated_at,
    };

    let saved_branch = current.clone();

    println!();
    println!(
        "  {} {} {} {}",
        "Folding:".bold().white(),
        child.green().bold(),
        "→".bright_black(),
        parent.cyan().bold()
    );
    if keep {
        println!(
            "  {} {}",
            "Keep:".bright_black(),
            "combined branch will keep the current branch name (--keep)".cyan()
        );
    }
    println!();

    if !keep {
        gitcmd::git_mutate(
            &["checkout", &parent],
            &format!(
                "Switch to parent branch '{}' to merge in '{}'",
                parent, child
            ),
        )
        .with_context(|| format!("Failed to checkout '{}'", parent))?;

        if let Err(e) = gitcmd::git_mutate(
            &["merge", &child],
            &format!(
                "Merge '{}' into '{}' (fast-forward when possible)",
                child, parent
            ),
        ) {
            if !gitcmd::is_dry_run() {
                let _ = gitcmd::git_output(&["merge", "--abort"]);
                let _ = gitcmd::git_output(&["checkout", &saved_branch]);
            }
            bail!("Merge failed: {}", e);
        }

        gitcmd::git_mutate(
            &["branch", "-d", &child],
            &format!(
                "Delete branch '{}' after it is merged into '{}'",
                child, parent
            ),
        )?;
    } else {
        gitcmd::git_mutate(
            &["checkout", &child],
            &format!(
                "Switch to '{}' to merge parent '{}' and keep this branch name",
                child, parent
            ),
        )
        .with_context(|| format!("Failed to checkout '{}'", child))?;

        if let Err(e) = gitcmd::git_mutate(
            &["merge", &parent],
            &format!(
                "Merge '{}' into '{}' so both histories are preserved on the kept branch",
                parent, child
            ),
        ) {
            if !gitcmd::is_dry_run() {
                let _ = gitcmd::git_output(&["merge", "--abort"]);
                let _ = gitcmd::git_output(&["checkout", &saved_branch]);
            }
            bail!("Merge failed: {}", e);
        }

        gitcmd::git_mutate(
            &["branch", "-d", &parent],
            &format!(
                "Delete parent branch '{}' after it is merged into '{}'",
                parent, child
            ),
        )?;
    }

    if !gitcmd::is_dry_run() {
        let stacks = repo_stacks_mut(&mut store, &repo);
        let st = stacks
            .iter_mut()
            .find(|s| s.name == stack_name)
            .with_context(|| format!("Stack '{}' not found after fold", stack_name))?;
        st.branches = new_branches.clone();
        st.root = new_root.clone();
        st.updated_at = Utc::now();
        save_store(&store)?;
    } else {
        gitcmd::dry_run_action(
            "Update stack metadata",
            &format!(
                "Rewrite stack '{}' branch list and root after fold",
                stack_name
            ),
        );
    }

    let restack_done = restack_branches_from(&restack_stack, restack_start, no_interactive)?;

    if !restack_done && !gitcmd::is_dry_run() {
        println!();
        ui::print_warning(&format!(
            "Fold merge finished; resolve the rebase conflict, then `{} stack sync` if needed.",
            crate::bin_name()
        ));
        println!();
        return Ok(());
    }

    gitcmd::git_mutate(
        &["checkout", &result_branch],
        &format!("Check out combined branch '{}'", result_branch),
    )?;

    if !gitcmd::is_dry_run() {
        println!();
        ui::print_success(&format!(
            "Folded {} into {}",
            child.green().bold(),
            result_branch.cyan().bold()
        ));
        println!();
    }

    Ok(())
}

/// Collapse all commits on the current branch to a single commit, then restack branches above.
///
/// Steps:
/// 1. `git reset --soft <base>` — stage all changes as one diff.
/// 2. `git commit -m <message>` — create the single squashed commit.
/// 3. Rebase each branch above the squashed one onto the new commit.
///
/// # Errors
///
/// Returns an error if the working tree is dirty, the branch is not in a stack,
/// the base branch does not exist locally, `<base>` is not an ancestor of the
/// current branch, or any git operation fails.
pub fn squash(message: Option<&str>, no_interactive: bool) -> Result<()> {
    let store = load_store()?;
    let cfg = config::load().unwrap_or_default();
    let repo = repo_id()?;
    let branch = gitcmd::current_branch()?;

    let (stack, pos) = {
        let stacks = repo_stacks(&store, &repo)
            .with_context(|| "No stacks in this repository.".to_string())?;
        let stack = find_stack_for_branch(stacks, &branch)
            .with_context(|| format!("Branch '{}' is not part of any stack.", branch))?;
        let pos = stack
            .branches
            .iter()
            .position(|b| b.name == branch)
            .with_context(|| format!("Branch '{}' not found in stack", branch))?;
        (stack.clone(), pos)
    };

    // The base is the branch immediately below, or the configured default branch.
    let base_ref = if pos > 0 {
        stack.branches[pos - 1].name.clone()
    } else {
        cfg.general.default_branch.clone()
    };

    gitcmd::require_clean_tree("squashing")?;

    gitcmd::git_output(&["rev-parse", "--verify", &base_ref]).with_context(|| {
        format!(
            "Base branch '{}' does not exist locally. For the bottom stack branch, set \
             [general].default_branch in config or create the branch.",
            base_ref
        )
    })?;

    if !gitcmd::is_ancestor(&base_ref, &branch)? {
        bail!(
            "'{}' is not an ancestor of '{}'. Run `{} stack sync` first, then try again.",
            base_ref,
            branch,
            crate::bin_name()
        );
    }

    let range = format!("{}..{}", base_ref, branch);
    let count: u32 = gitcmd::git_output(&["rev-list", "--count", &range])?
        .parse()
        .unwrap_or(0);
    if count == 0 {
        bail!(
            "There are no commits to squash on '{}' relative to '{}'.",
            branch,
            base_ref
        );
    }

    // Resolve commit message: explicit flag → oldest subject in range → fallback.
    let commit_msg = gitcmd::resolve_squash_message(message, &range, &branch)?;

    println!();
    println!(
        "  {} {} → {}",
        "Squashing branch:".bold().white(),
        branch.green().bold(),
        "one commit".cyan()
    );
    println!("  {} {}", "Base:".bright_black(), base_ref.cyan());
    println!();

    gitcmd::git_mutate(
        &["checkout", &branch],
        &format!("Switch to branch '{}' to squash commits", branch),
    )
    .with_context(|| format!("Failed to checkout '{}'", branch))?;

    gitcmd::git_mutate(
        &["reset", "--soft", &base_ref],
        &format!(
            "Soft-reset '{}' to '{}' (keep changes staged as one squashed commit)",
            branch, base_ref
        ),
    )
    .with_context(|| format!("Failed to reset '{}' to '{}'", branch, base_ref))?;

    gitcmd::git_mutate(
        &["commit", "-m", &commit_msg],
        "Create a single commit with the squashed changes",
    )
    .with_context(|| "Failed to commit squashed changes".to_string())?;

    // Rebase all branches above the squashed one.
    // We build a temporary Stack view starting at `pos + 1` and reuse the
    // existing `restack_branches_from` helper — this eliminates a third copy
    // of the rebase-with-conflict match block.
    let restack_done = restack_branches_from(&stack, pos + 1, no_interactive)?;

    if !restack_done {
        return Ok(());
    }

    gitcmd::git_mutate(
        &["checkout", &branch],
        &format!("Return to squashed branch '{}'", branch),
    )?;

    if !gitcmd::is_dry_run() {
        println!();
        ui::print_success(&format!(
            "Squashed {} onto {}",
            branch.green().bold(),
            base_ref.cyan()
        ));
        if pos + 1 < stack.branches.len() {
            ui::print_success("Restacked branches above.");
        }
        println!();
    }
    Ok(())
}

/// Rebase each branch in the current stack onto the one below it.
///
/// If a conflict occurs and `no_interactive` is `false`, the function pauses
/// and tells the user how to continue manually before returning `Ok(())`.
///
/// # Errors
///
/// Returns an error if the current branch is not in a stack, or if
/// `no_interactive` is `true` and a rebase conflict is detected.
pub fn sync(no_interactive: bool) -> Result<()> {
    let store = load_store()?;
    let stack = current_stack(&store)?.clone();

    ui::print_stack_banner("Syncing stack:", &stack.name);
    let saved_branch = gitcmd::current_branch()?;

    for i in 1..stack.branches.len() {
        let base = stack.branches[i - 1].name.clone();
        let branch = stack.branches[i].name.clone();

        gitcmd::git_mutate(
            &["checkout", &branch],
            &format!("Switch to branch '{}' to prepare for rebase", branch),
        )
        .with_context(|| format!("Failed to checkout '{}'", branch))?;

        let result = gitcmd::git_mutate(
            &["rebase", &base],
            &format!(
                "Rebase '{}' onto '{}' to incorporate latest changes from below",
                branch, base
            ),
        );

        match result {
            Ok(_) => {
                if !gitcmd::is_dry_run() {
                    ui::print_success(&format!(
                        "{} rebased onto {}",
                        branch.green().bold(),
                        base.cyan()
                    ));
                }
            }
            Err(e) => {
                if no_interactive {
                    let _ = gitcmd::git_output(&["rebase", "--abort"]);
                    bail!(
                        "Conflict rebasing '{}' onto '{}': {}\nRun without --no-interactive to resolve manually.",
                        branch, base, e
                    );
                } else {
                    print_conflict_instructions(&branch);
                    return Ok(());
                }
            }
        }
    }

    gitcmd::git_mutate(
        &["checkout", &saved_branch],
        &format!("Return to original branch '{}'", saved_branch),
    )?;

    if !gitcmd::is_dry_run() {
        println!();
        ui::print_success("Stack sync complete!");
        println!();
    }
    Ok(())
}

/// Push all branches in the current stack to `origin`.
///
/// Individual push failures are reported as errors but do not stop the loop —
/// all branches are attempted.
///
/// # Errors
///
/// Returns an error if the current branch is not part of any stack.
pub fn push(force: bool) -> Result<()> {
    let store = load_store()?;
    let stack = current_stack(&store)?.clone();

    ui::print_stack_banner("Pushing stack:", &stack.name);

    let force_note = if force {
        " with --force-with-lease"
    } else {
        ""
    };

    for branch_entry in &stack.branches {
        let branch = &branch_entry.name;

        let push_args: Vec<&str> = if force {
            vec!["push", "origin", branch, "--force-with-lease"]
        } else {
            vec!["push", "origin", branch]
        };

        let result = gitcmd::git_mutate(
            &push_args,
            &format!("Push branch '{}' to origin{}", branch, force_note),
        );

        match result {
            Ok(_) => {
                if !gitcmd::is_dry_run() {
                    ui::print_success(&format!("Pushed {}", branch.green().bold()));
                }
            }
            Err(e) => {
                ui::print_error(&format!("Failed to push {}: {}", branch.red(), e));
                if !force {
                    println!(
                        "  {}  try {}",
                        "tip:".bright_black(),
                        format!("{} stack push --force", crate::bin_name()).yellow()
                    );
                }
            }
        }
    }

    println!();
    Ok(())
}

/// Create or update GitHub PRs for every non-root branch in the current stack.
///
/// Each PR is chained so it targets the branch immediately below it.  Existing
/// PRs whose base branch has changed are updated automatically.
///
/// # Errors
///
/// Returns an error if the current branch is not in a stack, the GitHub token
/// is missing, or the repo owner/name cannot be detected.
pub fn create_prs(open: bool, draft: bool) -> Result<()> {
    let mut store = load_store()?;
    let repo = repo_id()?;
    let stack = current_stack(&store)?.clone();
    let cfg = config::load()?;

    let (owner, repo_name) = github::detect_repo()?;

    println!();
    println!(
        "  {} {} → {}/{}",
        "Creating PRs for stack:".bold().white(),
        stack.name.cyan().bold(),
        owner.bright_white(),
        repo_name.bright_white()
    );
    println!();

    if gitcmd::is_dry_run() {
        for i in 1..stack.branches.len() {
            let base = stack.branches[i - 1].name.clone();
            let branch = stack.branches[i].name.clone();
            let has_pr = stack.branches[i].pr_number.is_some();

            if has_pr {
                gitcmd::dry_run_action(
                    &format!("GitHub API: check/update PR for '{}'", branch),
                    &format!(
                        "Verify existing PR for '{}' → '{}' has correct base, update if needed",
                        branch, base
                    ),
                );
            } else {
                let draft_note = if draft { " as draft" } else { "" };
                gitcmd::dry_run_action(
                    &format!("GitHub API: create PR '{}' → '{}'", branch, base),
                    &format!(
                        "Open a new pull request{} from '{}' into '{}' on {}/{}",
                        draft_note, branch, base, owner, repo_name
                    ),
                );
            }
        }
        gitcmd::dry_run_action(
            "Save PR metadata",
            "Update stacks.toml with PR numbers and URLs",
        );
        return Ok(());
    }

    let token = get_github_token(&cfg)?;

    for i in 1..stack.branches.len() {
        let base = stack.branches[i - 1].name.clone();
        let branch = stack.branches[i].name.clone();

        let pb = ui::spinner(&format!(
            "Creating PR: {} → {}",
            branch.green(),
            base.cyan()
        ));

        let existing = github::find_pr(&token, &cfg.github.api_base, &owner, &repo_name, &branch)?;

        let result: Result<github::PrInfo> = if let Some(pr) = existing {
            if pr.base_ref != base {
                pb.set_message(format!(
                    "Updating PR #{} base: {} → {}",
                    pr.number,
                    pr.base_ref.red(),
                    base.green()
                ));
                let updated = github::update_pr_base(
                    &token,
                    &cfg.github.api_base,
                    &owner,
                    &repo_name,
                    pr.number,
                    &base,
                )?;
                pb.finish_and_clear();
                Ok(updated)
            } else {
                pb.finish_and_clear();
                Ok(pr)
            }
        } else {
            let pr_title = gitcmd::git_output_lossy(&["log", "--format=%s", "-1", &branch]);
            let title = if pr_title.is_empty() {
                branch.clone()
            } else {
                pr_title
            };
            let pr = github::create_pr(
                &token,
                &cfg.github.api_base,
                &owner,
                &repo_name,
                &title,
                &branch,
                &base,
                draft,
            )?;
            pb.finish_and_clear();
            Ok(pr)
        };

        match result {
            Ok(pr) => {
                let action = if stack.branches[i].pr_number.is_some() {
                    "Updated"
                } else {
                    "Created"
                };
                ui::print_success(&format!(
                    "{} PR #{}: {} → {}  {}",
                    action,
                    pr.number.to_string().yellow(),
                    branch.green().bold(),
                    base.cyan(),
                    pr.html_url.bright_black().underline()
                ));

                let stacks = repo_stacks_mut(&mut store, &repo);
                if let Some(s) = stacks.iter_mut().find(|s| s.name == stack.name) {
                    if let Some(b) = s.branches.iter_mut().find(|b| b.name == branch) {
                        b.pr_number = Some(pr.number);
                        b.pr_url = Some(pr.html_url.clone());
                    }
                }

                if open {
                    let _ = open_url(&pr.html_url);
                }
            }
            Err(e) => {
                ui::print_error(&format!("Failed to create PR for {}: {}", branch.red(), e));
            }
        }
    }

    save_store(&store)?;
    println!();
    Ok(())
}

/// Remove a branch from its stack without deleting the underlying git branch.
///
/// # Errors
///
/// Returns an error if the branch is not part of any stack or the store cannot
/// be saved.
pub fn remove_branch(branch: &str) -> Result<()> {
    let mut store = load_store()?;
    let repo = repo_id()?;

    let stacks = repo_stacks_mut(&mut store, &repo);

    let stack = stacks
        .iter_mut()
        .find(|s| s.branches.iter().any(|b| b.name == branch))
        .with_context(|| format!("Branch '{}' is not part of any stack", branch))?;

    let pos = stack
        .branches
        .iter()
        .position(|b| b.name == branch)
        .with_context(|| format!("Branch '{}' not found in stack", branch))?;

    if !gitcmd::is_dry_run() {
        stack.branches.remove(pos);
        save_store(&store)?;
        ui::print_success(&format!("Removed '{}' from stack", branch.yellow()));
    } else {
        gitcmd::dry_run_action(
            "Update stack metadata",
            &format!(
                "Remove branch '{}' (position {}) from stack in stacks.toml — git branch is not deleted",
                branch, pos
            ),
        );
    }
    Ok(())
}

/// Delete a stack entirely, optionally deleting all its git branches as well.
///
/// # Errors
///
/// Returns an error if the named stack is not found or the store cannot be saved.
pub fn delete_stack(name: &str, delete_branches: bool) -> Result<()> {
    let mut store = load_store()?;
    let repo = repo_id()?;

    let stacks = repo_stacks_mut(&mut store, &repo);

    let stack = stacks
        .iter()
        .find(|s| s.name == name)
        .cloned()
        .with_context(|| format!("Stack '{}' not found.", name))?;

    if delete_branches {
        for branch in &stack.branches {
            let result = gitcmd::git_mutate(
                &["branch", "-d", &branch.name],
                &format!(
                    "Delete branch '{}' (safe delete, only if fully merged)",
                    branch.name
                ),
            );
            if !gitcmd::is_dry_run() {
                if let Err(e) = result {
                    ui::print_warning(&format!("Could not delete branch '{}': {}", branch.name, e));
                }
            }
        }
    }

    if !gitcmd::is_dry_run() {
        stacks.retain(|s| s.name != name);
        save_store(&store)?;
        ui::print_success(&format!("Deleted stack '{}'", name.red()));
    } else {
        gitcmd::dry_run_action(
            "Delete stack metadata",
            &format!("Remove stack '{}' from stacks.toml", name),
        );
    }
    Ok(())
}

// ─── Stack ordering ────────────────────────────────────────────────────────────

/// Direction used by the internal [`move_branch`] helper.
enum Direction {
    /// Move the branch one step toward the bottom (lower index).
    Up,
    /// Move the branch one step toward the top (higher index).
    Down,
}

/// Swap the current branch one position in `direction` within the stack.
///
/// This affects display order and PR-chaining order only; it does **not** run
/// any `git rebase` or `git checkout`.
///
/// The `move_up` / `move_down` public functions are thin wrappers around this.
///
/// # Errors
///
/// Returns an error if the current branch is not in a stack or the store cannot
/// be saved.
fn move_branch(direction: Direction) -> Result<()> {
    let mut store = load_store()?;
    let repo = repo_id()?;
    let current_branch = gitcmd::current_branch()?;

    let stacks = repo_stacks_mut(&mut store, &repo);
    if stacks.is_empty() {
        bail!("No stacks in this repository.");
    }

    let stack = stacks
        .iter_mut()
        .find(|s| s.branches.iter().any(|b| b.name == current_branch))
        .with_context(|| format!("Branch '{}' is not part of any stack.", current_branch))?;

    let pos = stack
        .branches
        .iter()
        .position(|b| b.name == current_branch)
        .with_context(|| format!("Branch '{}' not found in stack", current_branch))?;

    let direction_label = match direction {
        Direction::Up => {
            if pos == 0 {
                println!();
                ui::print_warning("This is the bottom branch of the stack — cannot move up.");
                println!();
                return Ok(());
            }
            stack.branches.swap(pos, pos - 1);
            "up"
        }
        Direction::Down => {
            if pos == stack.branches.len() - 1 {
                println!();
                ui::print_warning("This is the top branch of the stack — cannot move down.");
                println!();
                return Ok(());
            }
            stack.branches.swap(pos, pos + 1);
            "down"
        }
    };

    save_store(&store)?;

    println!();
    ui::print_success(&format!(
        "Moved '{}' {} in the stack order",
        current_branch.green().bold(),
        direction_label
    ));
    println!();
    Ok(())
}

/// Move the current branch one position toward the bottom of the stack.
///
/// # Errors
///
/// Propagates any error from the internal [`move_branch`] helper.
pub fn move_up() -> Result<()> {
    move_branch(Direction::Up)
}

/// Move the current branch one position toward the top of the stack.
///
/// # Errors
///
/// Propagates any error from the internal [`move_branch`] helper.
pub fn move_down() -> Result<()> {
    move_branch(Direction::Down)
}
