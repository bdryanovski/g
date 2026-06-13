//! `g stack switch <name>` — check out the top branch of a stack.

use anyhow::{Context, Result};

use crate::commands::git as gitcmd;
use crate::commands::{Ctx, Error as CommandError};
use crate::storage::stacks as stacks_store;
use crate::ui;

use super::shared::current_repo_id;

/// Switch to the top branch of the named stack.
pub(super) fn run(ctx: &Ctx, name: &str) -> Result<()> {
    let conn = ctx.conn;
    let repo_id = current_repo_id(conn)?;
    let stacks = stacks_store::load_all(conn, repo_id)?;

    if stacks.is_empty() {
        return Err(CommandError::NoStacks.into());
    }

    let stack = stacks
        .iter()
        .find(|s| s.name == name || s.name.contains(name))
        .ok_or_else(|| CommandError::StackNotFound(name.to_string()))?;

    let top_branch = stack
        .branches
        .last()
        .with_context(|| format!("Stack '{}' has no branches.", name))?;

    let current = gitcmd::current_branch().unwrap_or_default();
    if current == top_branch.name && !gitcmd::is_dry_run() {
        ui::print_blank();
        ui::print_info(&format!(
            "Already on stack {} (branch {})",
            ui::primary_bold(&stack.name),
            ui::success_bold(&top_branch.name)
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
            ui::primary_bold(&stack.name),
            ui::success_bold(&top_branch.name)
        ));
        ui::print_blank();
    }
    Ok(())
}
