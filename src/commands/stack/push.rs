//! `g stack push` — push every branch of the current stack to `origin`.

use crate::commands::prelude::*;

use super::shared::current_stack;

/// Push all branches in the current stack to `origin`.
pub(super) fn run(ctx: &Ctx, force: bool) -> Result<()> {
    let conn = ctx.conn;
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
                    ui::print_success(&format!("Pushed {}", ui::success_bold(branch)));
                }
            }
            Err(e) => {
                ui::print_error(&format!("Failed to push {}: {}", ui::danger(branch), e));
                if !force {
                    ui::print_tip(&format!(
                        "try {} to force-push with lease",
                        ui::warning(&format!("{} stack push --force", crate::bin_name()))
                    ));
                }
            }
        }
    }

    ui::print_blank();
    Ok(())
}
