//! `g stack fold` — merge the current branch into its parent (or vice-versa
//! with `--keep`) and drop the now-redundant branch from the stack.

use anyhow::{bail, Context, Result};

use crate::commands::git as gitcmd;
use crate::commands::Ctx;
use crate::storage::{stacks as stacks_store, stats, StackRow};
use crate::ui;

use super::shared::{current_repo_id, find_stack_for_branch, positioned, restack_branches_from};

/// Merge the current branch into its parent (or vice-versa with `keep`) and
/// drop the now-redundant branch from the stack.
pub(super) fn run(ctx: &Ctx, keep: bool, no_interactive: bool) -> Result<()> {
    let conn = ctx.conn;
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
    ui::print_indented(&format!(
        "{} {} {} {}",
        ui::text_bold("Folding:"),
        ui::success_bold(&child),
        ui::muted("\u{2192}"),
        ui::primary_bold(&parent)
    ));
    if keep {
        ui::print_indented(&format!(
            "{} {}",
            ui::muted("Keep:"),
            ui::primary("combined branch will keep the current branch name (--keep)")
        ));
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
            ui::success_bold(&child),
            ui::primary_bold(&result_branch)
        ));
        ui::print_blank();
    }

    Ok(())
}
