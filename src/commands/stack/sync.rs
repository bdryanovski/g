//! `g stack sync` — rebase each branch onto the one below so the chain stays
//! consistent after upstream changes.

use anyhow::{bail, Context, Result};

use crate::commands::git as gitcmd;
use crate::commands::Ctx;
use crate::storage::stats;
use crate::ui;

use super::shared::{current_stack, print_conflict_instructions};

/// Rebase each branch in the current stack onto the one below it.
pub(super) fn run(ctx: &Ctx, no_interactive: bool) -> Result<()> {
    let conn = ctx.conn;
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
