//! `g stack new <name>` — create a new stack rooted at the current branch.

use crate::commands::prelude::*;
use crate::commands::Error as CommandError;
use crate::storage::{stacks as stacks_store, stats};

use super::shared::{current_repo_id, new_branch_row, positioned};

/// Create a new stack rooted at the current branch.
///
/// The current branch becomes the first entry in the stack's branch list and
/// also the `root_branch` (the eventual merge target for the entire chain).
pub(super) fn run(ctx: &Ctx, name: &str) -> Result<()> {
    let conn = ctx.conn;
    let repo_id = current_repo_id(conn)?;
    let branch = gitcmd::current_branch()?;
    let existing = stacks_store::load_all(conn, repo_id)?;

    if existing.iter().any(|s| s.name == name) {
        return Err(CommandError::StackExists(name.to_string()).into());
    }

    if !gitcmd::is_dry_run() {
        let stack_id = stacks_store::insert(conn, repo_id, name, &branch)?;
        let branches = positioned(vec![new_branch_row(&branch)]);
        stacks_store::set_branches(conn, stack_id, &branches)?;
        stats::record_stack_event(conn, Some(stack_id), Some(repo_id), "create").ok();

        ui::print_blank();
        ui::print_success(&format!(
            "Created stack {} rooted at {}",
            ui::primary_bold(name),
            ui::success_bold(&branch)
        ));
        ui::print_blank();
    } else {
        gitcmd::dry_run_action(
            "Create stack metadata",
            &format!(
                "Register stack '{}' rooted at branch '{}' in g.db",
                name, branch
            ),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::TestRepo;
    use crate::storage::stacks as stacks_store;

    #[test]
    fn creates_stack_rooted_at_current_branch() {
        let repo = TestRepo::new();
        let ctx = repo.ctx();

        run(&ctx, "feature-a").expect("g stack new should succeed");

        // One stack row exists, rooted at the current branch.
        let repo_id = current_repo_id(&repo.conn).unwrap();
        let stacks = stacks_store::load_all(&repo.conn, repo_id).unwrap();
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].name, "feature-a");
        assert_eq!(stacks[0].root_branch, "main");

        // The current branch is registered as the (only) stack branch.
        assert_eq!(stacks[0].branches.len(), 1);
        assert_eq!(stacks[0].branches[0].name, "main");
        assert_eq!(stacks[0].branches[0].position, 0);
    }

    #[test]
    fn rejects_duplicate_stack_name() {
        let repo = TestRepo::new();
        let ctx = repo.ctx();

        run(&ctx, "dup").expect("first create should succeed");
        let err = run(&ctx, "dup").expect_err("second create should fail");
        // Assert on the typed error kind, not the message text.
        match err.downcast_ref::<CommandError>() {
            Some(CommandError::StackExists(name)) => assert_eq!(name, "dup"),
            other => panic!("expected StackExists, got {other:?}"),
        }

        // The DB still holds exactly one stack.
        let repo_id = current_repo_id(&repo.conn).unwrap();
        let stacks = stacks_store::load_all(&repo.conn, repo_id).unwrap();
        assert_eq!(stacks.len(), 1);
    }

    #[test]
    fn different_names_coexist() {
        let repo = TestRepo::new();
        let ctx = repo.ctx();

        run(&ctx, "alpha").unwrap();
        run(&ctx, "beta").unwrap();

        let repo_id = current_repo_id(&repo.conn).unwrap();
        let names: Vec<_> = stacks_store::load_all(&repo.conn, repo_id)
            .unwrap()
            .into_iter()
            .map(|s| s.name)
            .collect();
        // Order isn't guaranteed by load_all; check membership.
        assert!(names.contains(&"alpha".to_string()));
        assert!(names.contains(&"beta".to_string()));
        assert_eq!(names.len(), 2);
    }
}
