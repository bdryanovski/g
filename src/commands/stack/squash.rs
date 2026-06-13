//! `g stack squash` — collapse all commits on the current branch into one,
//! then restack the branches above.

use crate::commands::prelude::*;

use super::shared::{current_stack, restack_branches_from};

/// Collapse all commits on the current branch to a single commit, then restack
/// branches above.
pub(super) fn run(ctx: &Ctx, message: Option<&str>, no_interactive: bool) -> Result<()> {
    let conn = ctx.conn;
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
    ui::print_indented(&format!(
        "{} {} \u{2192} {}",
        ui::text_bold("Squashing branch:"),
        ui::success_bold(&branch),
        ui::primary("one commit")
    ));
    ui::print_indented(&format!(
        "{} {}",
        ui::muted("Base:"),
        ui::primary(&base_ref)
    ));
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
            ui::success_bold(&branch),
            ui::primary(&base_ref)
        ));
        if pos + 1 < stack.branches.len() {
            ui::print_success("Restacked branches above.");
        }
        ui::print_blank();
    }
    Ok(())
}
