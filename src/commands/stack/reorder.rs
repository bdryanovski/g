//! `g stack up` / `g stack down` — swap the current branch one position
//! within its stack.

use anyhow::Result;

use crate::commands::git as gitcmd;
use rusqlite::Connection;

use crate::commands::{Ctx, Error as CommandError};
use crate::storage::stacks as stacks_store;
use crate::ui;

use super::shared::{current_repo_id, find_stack_for_branch, positioned};

/// Direction used by the internal [`move_branch`] helper.
enum Direction {
    /// Move the branch one step toward the bottom (lower index).
    Up,
    /// Move the branch one step toward the top (higher index).
    Down,
}

/// Swap the current branch one position in `direction` within the stack.
fn move_branch(conn: &Connection, direction: Direction) -> Result<()> {
    let repo_id = current_repo_id(conn)?;
    let current_branch = gitcmd::current_branch()?;
    let stacks = stacks_store::load_all(conn, repo_id)?;

    if stacks.is_empty() {
        return Err(CommandError::NoStacks.into());
    }

    let stack = find_stack_for_branch(&stacks, &current_branch)
        .ok_or_else(|| CommandError::BranchNotInStack(current_branch.clone()))?
        .clone();

    let pos = stack
        .branches
        .iter()
        .position(|b| b.name == current_branch)
        .ok_or_else(|| CommandError::BranchMissingFromStack(current_branch.clone()))?;

    let direction_label = match direction {
        Direction::Up => {
            if pos == 0 {
                ui::print_blank();
                ui::print_warning("This is the bottom branch of the stack — cannot move up.");
                ui::print_blank();
                return Ok(());
            }
            let mut new_branches = stack.branches.clone();
            new_branches.swap(pos, pos - 1);
            let new_branches = positioned(new_branches);
            stacks_store::set_branches(conn, stack.id, &new_branches)?;
            "up"
        }
        Direction::Down => {
            if pos == stack.branches.len() - 1 {
                ui::print_blank();
                ui::print_warning("This is the top branch of the stack — cannot move down.");
                ui::print_blank();
                return Ok(());
            }
            let mut new_branches = stack.branches.clone();
            new_branches.swap(pos, pos + 1);
            let new_branches = positioned(new_branches);
            stacks_store::set_branches(conn, stack.id, &new_branches)?;
            "down"
        }
    };

    ui::print_blank();
    ui::print_success(&format!(
        "Moved '{}' {} in the stack order",
        ui::success_bold(&current_branch),
        direction_label
    ));
    ui::print_blank();
    Ok(())
}

/// Move the current branch one position toward the bottom of the stack.
pub(super) fn up(ctx: &Ctx) -> Result<()> {
    let conn = ctx.conn;
    move_branch(conn, Direction::Up)
}

/// Move the current branch one position toward the top of the stack.
pub(super) fn down(ctx: &Ctx) -> Result<()> {
    let conn = ctx.conn;
    move_branch(conn, Direction::Down)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::stack::{add, new};
    use crate::commands::test_support::TestRepo;
    use crate::storage::stacks as stacks_store;

    /// Build a 3-branch stack `main → b2 → b3`, with the harness left on `b3`.
    fn build_three_branch_stack(repo: &TestRepo) {
        let ctx = repo.ctx();
        new::run(&ctx, "feat").unwrap();
        add::run(&ctx, "b2").unwrap();
        add::run(&ctx, "b3").unwrap();
    }

    fn branch_order(repo: &TestRepo) -> Vec<String> {
        let repo_id = current_repo_id(&repo.conn).unwrap();
        let stacks = stacks_store::load_all(&repo.conn, repo_id).unwrap();
        stacks[0].branches.iter().map(|b| b.name.clone()).collect()
    }

    #[test]
    fn down_swaps_with_branch_above() {
        let repo = TestRepo::new();
        build_three_branch_stack(&repo);
        // Currently on b3 (top). Move to b2 first, then `down` swaps b2 with b3.
        repo.checkout("b2");
        let ctx = repo.ctx();
        down(&ctx).expect("down should succeed");
        assert_eq!(branch_order(&repo), vec!["main", "b3", "b2"]);
    }

    #[test]
    fn up_swaps_with_branch_below() {
        let repo = TestRepo::new();
        build_three_branch_stack(&repo);
        // Move to b2, then `up` swaps b2 with main.
        repo.checkout("b2");
        let ctx = repo.ctx();
        up(&ctx).expect("up should succeed");
        assert_eq!(branch_order(&repo), vec!["b2", "main", "b3"]);
    }

    #[test]
    fn up_at_bottom_is_a_warning_not_a_swap() {
        let repo = TestRepo::new();
        build_three_branch_stack(&repo);
        // We're on b3 (top), but on `main` (bottom) `up` should no-op.
        repo.checkout("main");
        let before = branch_order(&repo);
        let ctx = repo.ctx();
        up(&ctx).expect("up at bottom should not error");
        assert_eq!(branch_order(&repo), before, "order should be unchanged");
    }

    #[test]
    fn down_at_top_is_a_warning_not_a_swap() {
        let repo = TestRepo::new();
        build_three_branch_stack(&repo);
        // Already on b3 (top); `down` should no-op.
        let before = branch_order(&repo);
        let ctx = repo.ctx();
        down(&ctx).expect("down at top should not error");
        assert_eq!(branch_order(&repo), before);
    }
}
