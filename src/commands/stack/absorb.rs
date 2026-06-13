//! `g stack absorb` — merge the current branch into the one below it.

use anyhow::{Context, Result};

use crate::commands::git as gitcmd;
use crate::commands::Ctx;
use crate::storage::{stacks as stacks_store, stats};
use crate::ui;

use super::shared::{current_repo_id, find_stack_for_branch, positioned};

/// Merge the current branch into the branch immediately below it in the stack.
///
/// The current branch is deleted after a successful `--no-ff` merge and is
/// removed from the stack metadata.
pub(super) fn run(ctx: &Ctx) -> Result<()> {
    let conn = ctx.conn;
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
            ui::success_bold(&absorbed_branch),
            ui::primary_bold(&target_branch)
        ));
        ui::print_line(&format!(
            "     {} Stack now has {} branch{}",
            ui::muted(""),
            ui::warning(&remaining.to_string()),
            if remaining == 1 { "" } else { "es" }
        ));
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
