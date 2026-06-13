//! `g stack delete <name> [--branches]` — drop a stack entirely (optionally
//! also deleting all of its git branches).

use crate::commands::prelude::*;
use crate::commands::Error as CommandError;
use crate::storage::{stacks as stacks_store, stats};

use super::shared::current_repo_id;

/// Delete a stack entirely, optionally deleting all its git branches as well.
pub(super) fn run(ctx: &Ctx, name: &str, delete_branches: bool) -> Result<()> {
    let conn = ctx.conn;
    let repo_id = current_repo_id(conn)?;
    let stacks = stacks_store::load_all(conn, repo_id)?;

    let stack = stacks
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| CommandError::StackNotFound(name.to_string()))?
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
        ui::print_success(&format!("Deleted stack '{}'", ui::danger(name)));
    } else {
        gitcmd::dry_run_action(
            "Delete stack metadata",
            &format!("Remove stack '{}' from g.db", name),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::stack::new;
    use crate::commands::test_support::TestRepo;
    use crate::storage::stacks as stacks_store;

    #[test]
    fn removes_stack_row_without_touching_git_branches() {
        let repo = TestRepo::new();
        let ctx = repo.ctx();
        new::run(&ctx, "feat").unwrap();
        run(&ctx, "feat", false).expect("delete should succeed");

        // The DB row is gone.
        let repo_id = current_repo_id(&repo.conn).unwrap();
        assert!(stacks_store::load_all(&repo.conn, repo_id)
            .unwrap()
            .is_empty());

        // The git branch (`main` was the root) is still around.
        let branches = repo.git(&["branch"]);
        assert!(
            branches.contains("main"),
            "main branch should still exist after delete: {branches:?}"
        );
    }

    #[test]
    fn fails_on_unknown_stack_name() {
        let repo = TestRepo::new();
        let ctx = repo.ctx();
        new::run(&ctx, "real").unwrap();
        let err = run(&ctx, "ghost", false).expect_err("must fail");
        match err.downcast_ref::<CommandError>() {
            Some(CommandError::StackNotFound(n)) => assert_eq!(n, "ghost"),
            other => panic!("expected StackNotFound, got {other:?}"),
        }
        // The legitimate stack is untouched.
        let repo_id = current_repo_id(&repo.conn).unwrap();
        assert_eq!(
            stacks_store::load_all(&repo.conn, repo_id).unwrap().len(),
            1
        );
    }
}
