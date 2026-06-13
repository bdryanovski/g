//! Cross-subcommand helpers used throughout the stack module.
//!
//! These are the "small but shared" building blocks: locating the current
//! stack, formatting branch rows, rebasing-above-a-position, talking to
//! GitHub, opening URLs and printing standard conflict instructions.

use anyhow::{bail, Context, Result};
use rusqlite::Connection;

use crate::commands::git as gitcmd;
use crate::commands::Error as CommandError;
use crate::config;
use crate::storage::{repos, stacks as stacks_store, StackBranchRow, StackRow};
use crate::ui;

/// Return the `repo_id` for the current git repository root (upserts the row).
pub(super) fn current_repo_id(conn: &Connection) -> Result<i64> {
    let root = gitcmd::repo_root()?;
    repos::upsert(conn, &root)
}

/// Return the stack that contains the current branch.
pub(super) fn current_stack(conn: &Connection) -> Result<StackRow> {
    let repo_id = current_repo_id(conn)?;
    let branch = gitcmd::current_branch()?;
    let stacks = stacks_store::load_all(conn, repo_id)?;

    stacks
        .into_iter()
        .find(|s| s.branches.iter().any(|b| b.name == branch))
        .ok_or_else(|| CommandError::BranchNotInStack(branch.clone()).into())
}

/// Find the stack that contains `branch` within a slice of stacks, or `None`.
pub(super) fn find_stack_for_branch<'a>(
    stacks: &'a [StackRow],
    branch: &str,
) -> Option<&'a StackRow> {
    stacks
        .iter()
        .find(|s| s.branches.iter().any(|b| b.name == branch))
}

/// Build a new [`StackBranchRow`] (position is set by [`positioned`]).
pub(super) fn new_branch_row(name: &str) -> StackBranchRow {
    StackBranchRow {
        position: 0,
        name: name.to_string(),
        pr_number: None,
        pr_url: None,
        description: None,
    }
}

/// Re-assign `position` values (0-based) to a branch slice in place.
pub(super) fn positioned(mut branches: Vec<StackBranchRow>) -> Vec<StackBranchRow> {
    for (i, b) in branches.iter_mut().enumerate() {
        b.position = i as i32;
    }
    branches
}

/// Retrieve the GitHub token from `GITHUB_TOKEN` env var or the config file.
pub(super) fn get_github_token(cfg: &config::Config) -> Result<String> {
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
pub(super) fn open_url(url: &str) -> Result<()> {
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

/// Print the standard "rebase conflict" instructions to the terminal.
pub(super) fn print_conflict_instructions(branch: &str) {
    let cmd = crate::bin_name();
    ui::print_warning(&format!(
        "Conflict in {}: resolve manually, then run `{} stack sync` again",
        ui::warning(branch),
        cmd
    ));
    ui::print_blank();
    ui::print_info("After resolving conflicts:");
    ui::print_step(1, 3, "git add <files>");
    ui::print_step(2, 3, "git rebase --continue");
    ui::print_step(3, 3, &format!("{cmd} stack sync  (to continue restacking)"));
    ui::print_blank();
}

/// Rebase each branch from index `start` upward onto the one below it.
///
/// Returns `Ok(true)` when all rebases completed without conflicts.
/// Returns `Ok(false)` when a conflict was found and `no_interactive` is `false`.
pub(super) fn restack_branches_from(
    stack: &StackRow,
    start: usize,
    no_interactive: bool,
) -> Result<bool> {
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
                        ui::success_bold(&branch),
                        ui::primary(&base)
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
