//! Stacked PR workflow management.
//!
//! ## Overview
//!
//! This module implements the "Stacked Pull Requests" workflow.  It tracks
//! ordered lists of branches (called *stacks*) in the `stacks` and
//! `stack_branches` tables of `~/.config/g/g.db`.
//!
//! Key operations:
//! - `new`    — start a stack rooted at the current branch.
//! - `add`    — append a new branch on top of the current stack position.
//! - `squash` — collapse the current branch to a single commit, then restack above.
//! - `fold`   — merge the current branch into its parent, then restack above.
//! - `sync`   — rebase the whole chain so each branch sits cleanly on the one below.
//! - `pr`     — create or update GitHub PRs so each PR targets the branch below.

use anyhow::{bail, Context, Result};
use colored::Colorize;
use rusqlite::Connection;

use crate::commands::git as gitcmd;
use crate::config;
use crate::github;
use crate::storage::{repos, stacks as stacks_store, stats, StackBranchRow, StackRow};
use crate::ui;

// ─── Internal: repo + stack helpers ──────────────────────────────────────────

/// Return the `repo_id` for the current git repository root (upserts the row).
///
/// # Errors
///
/// Returns an error if `git rev-parse --show-toplevel` fails.
fn current_repo_id(conn: &Connection) -> Result<i64> {
    let root = gitcmd::repo_root()?;
    repos::upsert(conn, &root)
}

/// Return the stack that contains the current branch.
///
/// # Errors
///
/// Returns an error if there are no stacks or the current branch is not in any stack.
fn current_stack(conn: &Connection) -> Result<StackRow> {
    let repo_id = current_repo_id(conn)?;
    let branch = gitcmd::current_branch()?;
    let stacks = stacks_store::load_all(conn, repo_id)?;

    stacks
        .into_iter()
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
fn find_stack_for_branch<'a>(stacks: &'a [StackRow], branch: &str) -> Option<&'a StackRow> {
    stacks
        .iter()
        .find(|s| s.branches.iter().any(|b| b.name == branch))
}

/// Build a new [`StackBranchRow`] (position is set by [`positioned`]).
fn new_branch(name: &str) -> StackBranchRow {
    StackBranchRow {
        position: 0,
        name: name.to_string(),
        pr_number: None,
        pr_url: None,
        description: None,
    }
}

/// Re-assign `position` values (0-based) to a branch slice in place.
fn positioned(mut branches: Vec<StackBranchRow>) -> Vec<StackBranchRow> {
    for (i, b) in branches.iter_mut().enumerate() {
        b.position = i as i32;
    }
    branches
}

/// Retrieve the GitHub token from `GITHUB_TOKEN` env var or the config file.
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
fn print_conflict_instructions(branch: &str) {
    let cmd = crate::bin_name();
    ui::print_warning(&format!(
        "Conflict in {}: resolve manually, then run `{} stack sync` again",
        branch.yellow(),
        cmd
    ));
    ui::print_blank();
    ui::print_info("After resolving conflicts:");
    ui::print_step(1, 3, "git add <files>");
    ui::print_step(2, 3, "git rebase --continue");
    ui::print_step(3, 3, &format!("{cmd} stack sync  (to continue restacking)"));
    ui::print_blank();
}

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Create a new stack rooted at the current branch.
///
/// The current branch becomes the first entry in the stack's branch list and
/// also the `root_branch` (the eventual merge target for the entire chain).
///
/// # Errors
///
/// Returns an error if a stack with the same name already exists.
pub fn new_stack(conn: &Connection, name: &str) -> Result<()> {
    let repo_id = current_repo_id(conn)?;
    let branch = gitcmd::current_branch()?;
    let existing = stacks_store::load_all(conn, repo_id)?;

    if existing.iter().any(|s| s.name == name) {
        bail!("Stack '{}' already exists in this repository.", name);
    }

    if !gitcmd::is_dry_run() {
        let stack_id = stacks_store::insert(conn, repo_id, name, &branch)?;
        let branches = positioned(vec![new_branch(&branch)]);
        stacks_store::set_branches(conn, stack_id, &branches)?;
        stats::record_stack_event(conn, Some(stack_id), Some(repo_id), "create").ok();

        ui::print_blank();
        ui::print_success(&format!(
            "Created stack {} rooted at {}",
            name.cyan().bold(),
            branch.green().bold()
        ));
        ui::print_blank();
    } else {
        gitcmd::dry_run_action(
            "Create stack metadata",
            &format!(
                "Register stack '{}' rooted at branch '{}' in g.db",
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
/// Returns an error if the current branch is not in a stack or `git checkout -b` fails.
pub fn add_branch(conn: &Connection, branch_name: &str) -> Result<()> {
    let repo_id = current_repo_id(conn)?;
    let current_branch = gitcmd::current_branch()?;
    let stack = current_stack(conn)?;

    let current_pos = stack
        .branches
        .iter()
        .position(|b| b.name == current_branch)
        .with_context(|| format!("Branch '{}' not found in stack", current_branch))?;

    gitcmd::git_mutate(
        &["checkout", "-b", branch_name],
        &format!(
            "Create new branch '{}' from current HEAD and switch to it",
            branch_name
        ),
    )
    .with_context(|| format!("Failed to create branch '{}'", branch_name))?;

    if !gitcmd::is_dry_run() {
        // Reload in case the stack was modified concurrently.
        let stack = stacks_store::load_by_name(conn, repo_id, &stack.name)?
            .with_context(|| format!("Stack '{}' disappeared after branch creation", stack.name))?;

        let mut new_branches = stack.branches.clone();
        let new_entry = new_branch(branch_name);
        new_branches.insert(current_pos + 1, new_entry);
        let new_branches = positioned(new_branches);
        stacks_store::set_branches(conn, stack.id, &new_branches)?;
        stats::record_stack_event(conn, Some(stack.id), Some(repo_id), "add").ok();
        stats::record_branch_event(conn, repo_id, branch_name, "create").ok();

        ui::print_blank();
        ui::print_success(&format!(
            "Created branch {} and added to stack",
            branch_name.green().bold()
        ));
        ui::print_blank();
    } else {
        gitcmd::dry_run_action(
            "Update stack metadata",
            &format!(
                "Insert '{}' into stack '{}' at position {}",
                branch_name,
                stack.name,
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
pub fn list(conn: &Connection) -> Result<()> {
    let repo_id = current_repo_id(conn)?;
    let stacks = stacks_store::load_all(conn, repo_id)?;
    let current_branch = gitcmd::current_branch().unwrap_or_default();

    if stacks.is_empty() {
        ui::print_blank();
        ui::print_info("No stacks yet.");
        ui::print_tip(&format!(
            "{} stack new <name>  to create a stack from the current branch",
            crate::bin_name()
        ));
        ui::print_blank();
        return Ok(());
    }

    for stack in &stacks {
        ui::print_blank();
        println!(
            "  {} {}  {}",
            "Stack:".bright_black(),
            stack.name.cyan().bold(),
            format!("(root: {})", stack.root_branch).bright_black()
        );
        ui::print_blank();

        let last = stack.branches.len().saturating_sub(1);
        for (i, branch) in stack.branches.iter().enumerate() {
            let is_current = branch.name == current_branch;
            let connector = if i == last {
                "  \u{2514}\u{2500}\u{2500}"
            } else {
                "  \u{251c}\u{2500}\u{2500}"
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
                print!("  {}", "\u{2190} you are here".bright_black());
            }
            ui::print_blank();

            if i < last {
                println!(
                    "  {}   {}",
                    "\u{2502}".bright_black(),
                    "\u{2502}".bright_black()
                );
            }
        }
    }
    ui::print_blank();
    Ok(())
}

/// Alias for [`list`] — shows the current stack tree.
///
/// # Errors
///
/// Propagates any error from [`list`].
pub fn view(conn: &Connection) -> Result<()> {
    list(conn)
}

/// Show the current stack with per-branch commit details and live PR status.
///
/// # Errors
///
/// Returns an error if the store cannot be read.
pub fn details(conn: &Connection) -> Result<()> {
    let stack = current_stack(conn)?;
    let current_branch = gitcmd::current_branch().unwrap_or_default();

    let open_prs = fetch_open_prs_for_details();

    ui::print_blank();
    println!(
        "  {} {}  {}",
        "Stack:".bright_black(),
        stack.name.cyan().bold(),
        format!("(root: {})", stack.root_branch).bright_black()
    );
    ui::print_blank();

    let last_branch = stack.branches.len().saturating_sub(1);

    for (i, branch) in stack.branches.iter().enumerate() {
        let is_current = branch.name == current_branch;
        let connector = if i == last_branch {
            "\u{2514}\u{2500}\u{2500}"
        } else {
            "\u{251c}\u{2500}\u{2500}"
        };
        let pipe = if i == last_branch { " " } else { "\u{2502}" };

        let marker = ui::branch_marker(is_current);
        let name_colored = ui::branch_name_colored(&branch.name, is_current);

        print!("  {} {} {}", connector.bright_black(), marker, name_colored);
        if is_current {
            print!("  {}", "(current)".green().dimmed());
        }
        ui::print_blank();

        let branch_time = gitcmd::git_output_lossy(&["log", "-1", "--format=%ar", &branch.name]);
        if !branch_time.is_empty() {
            println!(
                "  {}     {}",
                pipe.bright_black(),
                branch_time.trim().bright_black()
            );
        }

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

        let base = if i == 0 {
            &stack.root_branch
        } else {
            &stack.branches[i - 1].name
        };

        if i > 0 || branch.name != stack.root_branch {
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
            println!(
                "  {}   {}",
                "\u{2502}".bright_black(),
                "\u{2502}".bright_black()
            );
        }
    }

    ui::print_blank();
    Ok(())
}

/// Try to fetch all open PRs from GitHub for display in [`details`].
///
/// Returns `None` silently when no token is configured or the API call fails.
fn fetch_open_prs_for_details() -> Option<std::collections::HashMap<String, github::PrInfo>> {
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
pub fn switch_stack(conn: &Connection, name: &str) -> Result<()> {
    let repo_id = current_repo_id(conn)?;
    let stacks = stacks_store::load_all(conn, repo_id)?;

    if stacks.is_empty() {
        bail!("No stacks in this repository.");
    }

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
        ui::print_blank();
        ui::print_info(&format!(
            "Already on stack {} (branch {})",
            stack.name.cyan().bold(),
            top_branch.name.green().bold()
        ));
        ui::print_blank();
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
        ui::print_blank();
        ui::print_success(&format!(
            "Switched to stack {} \u{2192} branch {}",
            stack.name.cyan().bold(),
            top_branch.name.green().bold()
        ));
        ui::print_blank();
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
pub fn absorb(conn: &Connection) -> Result<()> {
    let repo_id = current_repo_id(conn)?;
    let current_branch = gitcmd::current_branch()?;
    let stacks = stacks_store::load_all(conn, repo_id)?;

    let stack = find_stack_for_branch(&stacks, &current_branch)
        .with_context(|| format!("Branch '{}' is not part of any stack.", current_branch))?
        .clone();

    let pos = stack
        .branches
        .iter()
        .position(|b| b.name == current_branch)
        .with_context(|| format!("Branch '{}' not found in stack", current_branch))?;

    if pos == 0 {
        ui::print_blank();
        ui::print_warning("This is the bottom branch of the stack — nothing below to absorb into.");
        ui::print_blank();
        return Ok(());
    }

    let target_branch = stack.branches[pos - 1].name.clone();
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
        let mut new_branches = stack.branches.clone();
        new_branches.remove(pos);
        let new_branches = positioned(new_branches);
        let remaining = new_branches.len();
        stacks_store::set_branches(conn, stack.id, &new_branches)?;
        stats::record_stack_event(conn, Some(stack.id), Some(stack.repo_id), "absorb").ok();
        stats::record_branch_event(conn, stack.repo_id, &absorbed_branch, "delete").ok();

        ui::print_blank();
        ui::print_success(&format!(
            "Absorbed {} into {}",
            absorbed_branch.green().bold(),
            target_branch.cyan().bold()
        ));
        println!(
            "     {} Stack now has {} branch{}",
            "".bright_black(),
            remaining.to_string().yellow(),
            if remaining == 1 { "" } else { "es" }
        );
        ui::print_blank();
    } else {
        gitcmd::dry_run_action(
            "Update stack metadata",
            &format!(
                "Remove '{}' from stack '{}' in g.db",
                absorbed_branch, stack.name
            ),
        );
    }
    Ok(())
}

/// Rebase each branch from index `start` upward onto the one below it.
///
/// Returns `Ok(true)` when all rebases completed without conflicts.
/// Returns `Ok(false)` when a conflict was found and `no_interactive` is `false`.
fn restack_branches_from(stack: &StackRow, start: usize, no_interactive: bool) -> Result<bool> {
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

/// Merge the current branch into its parent (or vice-versa with `keep`) and
/// drop the now-redundant branch from the stack.
///
/// # Errors
///
/// Returns an error if the working tree is dirty, the current branch is at the
/// bottom of the stack, or any git operation fails.
pub fn fold(conn: &Connection, keep: bool, no_interactive: bool) -> Result<()> {
    let repo_id = current_repo_id(conn)?;
    let current = gitcmd::current_branch()?;
    let stacks = stacks_store::load_all(conn, repo_id)?;

    let stack = find_stack_for_branch(&stacks, &current)
        .with_context(|| format!("Branch '{}' is not part of any stack.", current))?
        .clone();

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

    gitcmd::require_clean_tree("folding")?;

    let (result_branch, restack_start, new_branches, new_root) = if !keep {
        let mut nb = stack.branches.clone();
        nb.remove(pos);
        (parent.clone(), pos, nb, stack.root_branch.clone())
    } else {
        let mut nb = stack.branches.clone();
        nb.remove(pos - 1);
        let nr = if stack.root_branch == parent {
            child.clone()
        } else {
            stack.root_branch.clone()
        };
        (child.clone(), pos, nb, nr)
    };

    let new_branches = positioned(new_branches);

    // Build a temporary StackRow for restack_branches_from.
    let restack_stack = StackRow {
        id: stack.id,
        repo_id: stack.repo_id,
        name: stack.name.clone(),
        root_branch: new_root.clone(),
        created_at: stack.created_at,
        updated_at: stack.updated_at,
        branches: new_branches.clone(),
    };

    let saved_branch = current.clone();

    ui::print_blank();
    println!(
        "  {} {} {} {}",
        "Folding:".bold().white(),
        child.green().bold(),
        "\u{2192}".bright_black(),
        parent.cyan().bold()
    );
    if keep {
        println!(
            "  {} {}",
            "Keep:".bright_black(),
            "combined branch will keep the current branch name (--keep)".cyan()
        );
    }
    ui::print_blank();

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
        stacks_store::set_branches(conn, stack.id, &new_branches)?;
        stats::record_stack_event(conn, Some(stack.id), Some(stack.repo_id), "fold").ok();
        // Record the branch that was deleted: `child` when !keep, `parent` when keep.
        let deleted_branch = if keep { &parent } else { &child };
        stats::record_branch_event(conn, stack.repo_id, deleted_branch, "delete").ok();
    } else {
        gitcmd::dry_run_action(
            "Update stack metadata",
            &format!(
                "Rewrite stack '{}' branch list and root after fold",
                stack.name
            ),
        );
    }

    let restack_done = restack_branches_from(&restack_stack, restack_start, no_interactive)?;

    if !restack_done && !gitcmd::is_dry_run() {
        ui::print_blank();
        ui::print_warning(&format!(
            "Fold merge finished; resolve the rebase conflict, then `{} stack sync` if needed.",
            crate::bin_name()
        ));
        ui::print_blank();
        return Ok(());
    }

    gitcmd::git_mutate(
        &["checkout", &result_branch],
        &format!("Check out combined branch '{}'", result_branch),
    )?;

    if !gitcmd::is_dry_run() {
        ui::print_blank();
        ui::print_success(&format!(
            "Folded {} into {}",
            child.green().bold(),
            result_branch.cyan().bold()
        ));
        ui::print_blank();
    }

    Ok(())
}

/// Collapse all commits on the current branch to a single commit, then restack branches above.
///
/// # Errors
///
/// Returns an error if the working tree is dirty, the branch is not in a stack,
/// or any git operation fails.
pub fn squash(conn: &Connection, message: Option<&str>, no_interactive: bool) -> Result<()> {
    let cfg = config::load().unwrap_or_default();
    let stack = current_stack(conn)?;
    let branch = gitcmd::current_branch()?;

    let pos = stack
        .branches
        .iter()
        .position(|b| b.name == branch)
        .with_context(|| format!("Branch '{}' not found in stack", branch))?;

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

    let commit_msg = gitcmd::resolve_squash_message(message, &range, &branch)?;

    ui::print_blank();
    println!(
        "  {} {} \u{2192} {}",
        "Squashing branch:".bold().white(),
        branch.green().bold(),
        "one commit".cyan()
    );
    println!("  {} {}", "Base:".bright_black(), base_ref.cyan());
    ui::print_blank();

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

    let restack_done = restack_branches_from(&stack, pos + 1, no_interactive)?;

    if !restack_done {
        return Ok(());
    }

    gitcmd::git_mutate(
        &["checkout", &branch],
        &format!("Return to squashed branch '{}'", branch),
    )?;

    if !gitcmd::is_dry_run() {
        ui::print_blank();
        ui::print_success(&format!(
            "Squashed {} onto {}",
            branch.green().bold(),
            base_ref.cyan()
        ));
        if pos + 1 < stack.branches.len() {
            ui::print_success("Restacked branches above.");
        }
        ui::print_blank();
    }
    Ok(())
}

/// Rebase each branch in the current stack onto the one below it.
///
/// # Errors
///
/// Returns an error if the current branch is not in a stack.
pub fn sync(conn: &Connection, no_interactive: bool) -> Result<()> {
    let stack = current_stack(conn)?;

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
        // Best-effort stats — look up the stack to get its ID.
        if let Ok(s) = current_stack(conn) {
            stats::record_stack_event(conn, Some(s.id), Some(s.repo_id), "sync").ok();
        }
        ui::print_blank();
        ui::print_success("Stack sync complete!");
        ui::print_blank();
    }
    Ok(())
}

/// Push all branches in the current stack to `origin`.
///
/// # Errors
///
/// Returns an error if the current branch is not part of any stack.
pub fn push(conn: &Connection, force: bool) -> Result<()> {
    let stack = current_stack(conn)?;

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
                    ui::print_tip(&format!(
                        "try {} to force-push with lease",
                        format!("{} stack push --force", crate::bin_name()).yellow()
                    ));
                }
            }
        }
    }

    ui::print_blank();
    Ok(())
}

/// Create or update GitHub PRs for every non-root branch in the current stack.
///
/// # Errors
///
/// Returns an error if the current branch is not in a stack, the GitHub token
/// is missing, or the repo owner/name cannot be detected.
pub fn create_prs(conn: &Connection, open: bool, draft: bool) -> Result<()> {
    let stack = current_stack(conn)?;
    let cfg = config::load()?;

    let (owner, repo_name) = github::detect_repo()?;

    ui::print_blank();
    println!(
        "  {} {} \u{2192} {}/{}",
        "Creating PRs for stack:".bold().white(),
        stack.name.cyan().bold(),
        owner.bright_white(),
        repo_name.bright_white()
    );
    ui::print_blank();

    if gitcmd::is_dry_run() {
        for i in 1..stack.branches.len() {
            let base = stack.branches[i - 1].name.clone();
            let branch = stack.branches[i].name.clone();
            let has_pr = stack.branches[i].pr_number.is_some();

            if has_pr {
                gitcmd::dry_run_action(
                    &format!("GitHub API: check/update PR for '{}'", branch),
                    &format!(
                        "Verify existing PR for '{}' \u{2192} '{}' has correct base, update if needed",
                        branch, base
                    ),
                );
            } else {
                let draft_note = if draft { " as draft" } else { "" };
                gitcmd::dry_run_action(
                    &format!("GitHub API: create PR '{}' \u{2192} '{}'", branch, base),
                    &format!(
                        "Open a new pull request{} from '{}' into '{}' on {}/{}",
                        draft_note, branch, base, owner, repo_name
                    ),
                );
            }
        }
        gitcmd::dry_run_action("Save PR metadata", "Update g.db with PR numbers and URLs");
        return Ok(());
    }

    let token = get_github_token(&cfg)?;

    for i in 1..stack.branches.len() {
        let base = stack.branches[i - 1].name.clone();
        let branch = stack.branches[i].name.clone();

        let pb = ui::spinner(&format!(
            "Creating PR: {} \u{2192} {}",
            branch.green(),
            base.cyan()
        ));

        let existing = github::find_pr(&token, &cfg.github.api_base, &owner, &repo_name, &branch)?;

        let result: Result<github::PrInfo> = if let Some(pr) = existing {
            if pr.base_ref != base {
                pb.set_message(format!(
                    "Updating PR #{} base: {} \u{2192} {}",
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
                    "{} PR #{}: {} \u{2192} {}  {}",
                    action,
                    pr.number.to_string().yellow(),
                    branch.green().bold(),
                    base.cyan(),
                    pr.html_url.bright_black().underline()
                ));

                stacks_store::update_branch_pr(conn, stack.id, &branch, pr.number, &pr.html_url)
                    .ok(); // best-effort

                if open {
                    let _ = open_url(&pr.html_url);
                }
            }
            Err(e) => {
                ui::print_error(&format!("Failed to create PR for {}: {}", branch.red(), e));
            }
        }
    }

    ui::print_blank();
    Ok(())
}

/// Remove a branch from its stack without deleting the underlying git branch.
///
/// # Errors
///
/// Returns an error if the branch is not part of any stack.
pub fn remove_branch(conn: &Connection, branch: &str) -> Result<()> {
    let repo_id = current_repo_id(conn)?;
    let stacks = stacks_store::load_all(conn, repo_id)?;

    let stack = find_stack_for_branch(&stacks, branch)
        .with_context(|| format!("Branch '{}' is not part of any stack", branch))?
        .clone();

    let pos = stack
        .branches
        .iter()
        .position(|b| b.name == branch)
        .with_context(|| format!("Branch '{}' not found in stack", branch))?;

    if !gitcmd::is_dry_run() {
        let mut new_branches = stack.branches.clone();
        new_branches.remove(pos);
        let new_branches = positioned(new_branches);
        stacks_store::set_branches(conn, stack.id, &new_branches)?;
        ui::print_success(&format!("Removed '{}' from stack", branch.yellow()));
    } else {
        gitcmd::dry_run_action(
            "Update stack metadata",
            &format!(
                "Remove branch '{}' (position {}) from stack in g.db — git branch is not deleted",
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
/// Returns an error if the named stack is not found.
pub fn delete_stack(conn: &Connection, name: &str, delete_branches: bool) -> Result<()> {
    let repo_id = current_repo_id(conn)?;
    let stacks = stacks_store::load_all(conn, repo_id)?;

    let stack = stacks
        .iter()
        .find(|s| s.name == name)
        .with_context(|| format!("Stack '{}' not found.", name))?
        .clone();

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
        stats::record_stack_event(conn, Some(stack.id), Some(stack.repo_id), "delete").ok();
        stacks_store::delete(conn, stack.id)?;
        ui::print_success(&format!("Deleted stack '{}'", name.red()));
    } else {
        gitcmd::dry_run_action(
            "Delete stack metadata",
            &format!("Remove stack '{}' from g.db", name),
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
fn move_branch(conn: &Connection, direction: Direction) -> Result<()> {
    let repo_id = current_repo_id(conn)?;
    let current_branch = gitcmd::current_branch()?;
    let stacks = stacks_store::load_all(conn, repo_id)?;

    if stacks.is_empty() {
        bail!("No stacks in this repository.");
    }

    let stack = find_stack_for_branch(&stacks, &current_branch)
        .with_context(|| format!("Branch '{}' is not part of any stack.", current_branch))?
        .clone();

    let pos = stack
        .branches
        .iter()
        .position(|b| b.name == current_branch)
        .with_context(|| format!("Branch '{}' not found in stack", current_branch))?;

    let direction_label = match direction {
        Direction::Up => {
            if pos == 0 {
                ui::print_blank();
                ui::print_warning("This is the bottom branch of the stack — cannot move up.");
                ui::print_blank();
                return Ok(());
            }
            let mut new_branches = stack.branches.clone();
            new_branches.swap(pos, pos - 1);
            let new_branches = positioned(new_branches);
            stacks_store::set_branches(conn, stack.id, &new_branches)?;
            "up"
        }
        Direction::Down => {
            if pos == stack.branches.len() - 1 {
                ui::print_blank();
                ui::print_warning("This is the top branch of the stack — cannot move down.");
                ui::print_blank();
                return Ok(());
            }
            let mut new_branches = stack.branches.clone();
            new_branches.swap(pos, pos + 1);
            let new_branches = positioned(new_branches);
            stacks_store::set_branches(conn, stack.id, &new_branches)?;
            "down"
        }
    };

    ui::print_blank();
    ui::print_success(&format!(
        "Moved '{}' {} in the stack order",
        current_branch.green().bold(),
        direction_label
    ));
    ui::print_blank();
    Ok(())
}

/// Move the current branch one position toward the bottom of the stack.
///
/// # Errors
///
/// Propagates any error from the internal [`move_branch`] helper.
pub fn move_up(conn: &Connection) -> Result<()> {
    move_branch(conn, Direction::Up)
}

/// Move the current branch one position toward the top of the stack.
///
/// # Errors
///
/// Propagates any error from the internal [`move_branch`] helper.
pub fn move_down(conn: &Connection) -> Result<()> {
    move_branch(conn, Direction::Down)
}
