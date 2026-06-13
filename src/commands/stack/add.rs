//! `g stack add <branch>` — create a new branch above the current position
//! and add it to the stack.

use crate::commands::prelude::*;
use crate::storage::{stacks as stacks_store, stats};

use super::shared::{current_repo_id, current_stack, new_branch_row, positioned};

/// Create a new branch directly above the current stack position and add it
/// to the stack. The new branch is checked out immediately.
pub(super) fn run(ctx: &Ctx, branch_name: &str) -> Result<()> {
    let conn = ctx.conn;
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
        let new_entry = new_branch_row(branch_name);
        new_branches.insert(current_pos + 1, new_entry);
        let new_branches = positioned(new_branches);
        stacks_store::set_branches(conn, stack.id, &new_branches)?;
        stats::record_stack_event(conn, Some(stack.id), Some(repo_id), "add").ok();
        stats::record_branch_event(conn, repo_id, branch_name, "create").ok();

        ui::print_blank();
        ui::print_success(&format!(
            "Created branch {} and added to stack",
            ui::success_bold(branch_name)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::stack::new;
    use crate::commands::test_support::TestRepo;
    use crate::commands::Error as CommandError;
    use crate::storage::stacks as stacks_store;

    /// Helper: returns the branch list for the single stack in the repo.
    fn branches_of_only_stack(repo: &TestRepo) -> Vec<String> {
        let repo_id = current_repo_id(&repo.conn).unwrap();
        let stacks = stacks_store::load_all(&repo.conn, repo_id).unwrap();
        assert_eq!(stacks.len(), 1, "expected exactly one stack");
        stacks[0].branches.iter().map(|b| b.name.clone()).collect()
    }

    #[test]
    fn appends_branch_above_current_position() {
        let repo = TestRepo::new();
        let ctx = repo.ctx();
        new::run(&ctx, "feat").unwrap();
        // We're on `main` (position 0).  `add` should place `b2` at position 1.
        run(&ctx, "b2").unwrap();
        assert_eq!(branches_of_only_stack(&repo), vec!["main", "b2"]);
        // And `git checkout -b b2` actually happened.
        assert_eq!(repo.current_branch(), "b2");
    }

    #[test]
    fn inserts_between_existing_branches_when_back_at_main() {
        // Build the chain main → b2 → b3 by adding both from successive
        // branches, then move back to main and add b1.5 in the middle.
        let repo = TestRepo::new();
        let ctx = repo.ctx();
        new::run(&ctx, "feat").unwrap();
        run(&ctx, "b2").unwrap();
        run(&ctx, "b3").unwrap();
        assert_eq!(branches_of_only_stack(&repo), vec!["main", "b2", "b3"]);

        // Move back to `main` and add a new branch — it should land at index 1.
        repo.checkout("main");
        run(&ctx, "b1_5").unwrap();
        assert_eq!(
            branches_of_only_stack(&repo),
            vec!["main", "b1_5", "b2", "b3"]
        );
    }

    #[test]
    fn fails_when_not_inside_any_stack() {
        let repo = TestRepo::new();
        let ctx = repo.ctx();
        // No `g stack new` first — current branch isn't in any stack.
        let err = run(&ctx, "loose").expect_err("add should require a stack");
        // Assert on the typed error kind.
        match err.downcast_ref::<CommandError>() {
            Some(CommandError::BranchNotInStack(branch)) => assert_eq!(branch, "main"),
            other => panic!("expected BranchNotInStack, got {other:?}"),
        }
    }
}
