//! `g stack remove <branch>` — drop a branch from its stack metadata without
//! deleting the underlying git branch.

use anyhow::Result;

use crate::commands::git as gitcmd;
use crate::commands::{Ctx, Error as CommandError};
use crate::storage::stacks as stacks_store;
use crate::ui;

use super::shared::{current_repo_id, find_stack_for_branch, positioned};

/// Remove a branch from its stack without deleting the underlying git branch.
pub(super) fn run(ctx: &Ctx, branch: &str) -> Result<()> {
    let conn = ctx.conn;
    let repo_id = current_repo_id(conn)?;
    let stacks = stacks_store::load_all(conn, repo_id)?;

    let stack = find_stack_for_branch(&stacks, branch)
        .ok_or_else(|| CommandError::BranchNotInStack(branch.to_string()))?
        .clone();

    let pos = stack
        .branches
        .iter()
        .position(|b| b.name == branch)
        .ok_or_else(|| CommandError::BranchMissingFromStack(branch.to_string()))?;

    if !gitcmd::is_dry_run() {
        let mut new_branches = stack.branches.clone();
        new_branches.remove(pos);
        let new_branches = positioned(new_branches);
        stacks_store::set_branches(conn, stack.id, &new_branches)?;
        ui::print_success(&format!("Removed '{}' from stack", ui::warning(branch)));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::stack::{add, new};
    use crate::commands::test_support::TestRepo;
    use crate::storage::stacks as stacks_store;

    #[test]
    fn drops_branch_from_stack_metadata_but_keeps_git_branch() {
        let repo = TestRepo::new();
        let ctx = repo.ctx();
        new::run(&ctx, "feat").unwrap();
        add::run(&ctx, "b2").unwrap();

        run(&ctx, "b2").expect("remove should succeed");

        // The stack now only contains the original root branch.
        let repo_id = current_repo_id(&repo.conn).unwrap();
        let stacks = stacks_store::load_all(&repo.conn, repo_id).unwrap();
        assert_eq!(stacks.len(), 1);
        let names: Vec<_> = stacks[0].branches.iter().map(|b| &b.name).collect();
        assert_eq!(names, vec!["main"]);

        // …but the git branch itself still exists (remove is metadata-only).
        let branches = repo.git(&["branch", "--list", "b2"]);
        assert!(
            !branches.is_empty(),
            "git branch b2 should still exist after `g stack remove`"
        );
    }

    #[test]
    fn fails_when_branch_not_in_any_stack() {
        let repo = TestRepo::new();
        let ctx = repo.ctx();
        new::run(&ctx, "feat").unwrap();
        let err = run(&ctx, "ghost").expect_err("removing a non-stack branch must fail");
        match err.downcast_ref::<CommandError>() {
            Some(CommandError::BranchNotInStack(b)) => assert_eq!(b, "ghost"),
            other => panic!("expected BranchNotInStack, got {other:?}"),
        }
    }
}
