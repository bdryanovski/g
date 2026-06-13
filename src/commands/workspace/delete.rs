//! `g workspace delete <name> [--force]` — remove a workspace and its git
//! worktree.  Also cleans up DB metadata when the directory is already gone.

use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::commands::git as gitcmd;
use crate::commands::{Ctx, Error as CommandError};
use crate::storage::{stats, workspaces as ws_store};
use crate::ui;

use super::shared::{list_worktrees, load_repo_workspaces};

pub(super) fn run(ctx: &Ctx, name: &str, force: bool) -> Result<()> {
    let conn = ctx.conn;
    let (_, workspaces) = load_repo_workspaces(conn)?;

    // Resolve the workspace: first by name, then by branch name.
    let ws = workspaces
        .iter()
        .find(|w| w.name == name)
        .or_else(|| workspaces.iter().find(|w| w.branch == name));

    // If no DB record exists, check whether git still tracks a worktree with
    // a matching branch name (e.g. the DB row was removed but the worktree
    // was never pruned, causing `list` to show it as `(main)`).
    let (ws_id_opt, wt_path) = match ws {
        Some(ws) => (Some(ws.id), ws.path.clone()),
        None => {
            let worktrees = list_worktrees()?;
            let wt = worktrees
                .iter()
                .find(|wt| wt.branch.as_deref() == Some(name));
            match wt {
                Some(wt) => (None, wt.path.to_string_lossy().to_string()),
                None => return Err(CommandError::WorkspaceNotFound(name.to_string()).into()),
            }
        }
    };

    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(&wt_path);

    let force_note = if force { " (forced)" } else { "" };
    let result = gitcmd::git_mutate(
        &args,
        &format!("Remove worktree directory at {}{}", wt_path, force_note),
    );

    match result {
        Ok(_) => {}
        Err(e) => {
            if gitcmd::is_dry_run() {
                return Err(e);
            }
            let msg = format!("{}", e);
            if msg.contains("dirty") || msg.contains("untracked") {
                bail!(
                    "Worktree has uncommitted changes. Use `{} workspace delete {} --force` to remove anyway.",
                    crate::bin_name(),
                    name
                );
            }
            if !Path::new(&wt_path).exists() {
                // Directory is already gone — clean up git metadata and
                // continue so the DB record (if any) is also removed.
                ui::print_warning("Worktree directory already removed; cleaning up metadata.");
                gitcmd::git_output(&["worktree", "prune"]).ok();
            } else if msg.to_lowercase().contains("permission denied")
                || msg.contains("EPERM")
                || msg.contains("EACCES")
            {
                bail!(
                    "Cannot remove worktree at '{}': permission denied.\nCheck directory ownership and permissions.",
                    wt_path
                );
            } else {
                return Err(e).context("Failed to remove worktree");
            }
        }
    }

    if !gitcmd::is_dry_run() {
        if let Some(ws_id) = ws_id_opt {
            stats::record_workspace_event(conn, Some(ws_id), None, "delete").ok();
            ws_store::delete(conn, ws_id)?;
        }
        ui::print_blank();
        ui::print_success(&format!("Deleted workspace '{}'", ui::danger(name)));
        ui::print_blank();
    } else if ws_id_opt.is_some() {
        gitcmd::dry_run_action(
            "Remove workspace metadata",
            &format!("Delete workspace '{}' entry from g.db", name),
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::TestRepo;
    use crate::commands::workspace::create;

    #[test]
    fn removes_worktree_and_db_row() {
        let repo = TestRepo::new();
        let ctx = repo.ctx();
        create::run(&ctx, "tmp-ws", None, None, None, false).unwrap();

        let (_, before) = load_repo_workspaces(&repo.conn).unwrap();
        let row = before
            .iter()
            .find(|w| w.name == "tmp-ws")
            .expect("setup row")
            .clone();
        let wt_path = Path::new(&row.path);
        assert!(
            wt_path.exists(),
            "fixture worktree must exist before delete"
        );

        run(&ctx, "tmp-ws", false).expect("delete should succeed");

        // The worktree directory is gone…
        assert!(!wt_path.exists(), "worktree dir should be removed");
        // …and the DB row is gone too.
        let (_, after) = load_repo_workspaces(&repo.conn).unwrap();
        assert!(after.iter().all(|w| w.name != "tmp-ws"));
    }

    #[test]
    fn fails_on_unknown_workspace_name() {
        let repo = TestRepo::new();
        let ctx = repo.ctx();
        let err = run(&ctx, "ghost", false).expect_err("delete must fail");
        match err.downcast_ref::<CommandError>() {
            Some(CommandError::WorkspaceNotFound(n)) => assert_eq!(n, "ghost"),
            other => panic!("expected WorkspaceNotFound, got {other:?}"),
        }
    }
}
