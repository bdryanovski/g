//! `g workspace delete <name> [--force]` — remove a workspace and its git
//! worktree.  Also cleans up DB metadata when the directory is already gone.

use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

use crate::commands::git as gitcmd;
use crate::commands::{Ctx, Error as CommandError};
use crate::storage::{stats, workspaces as ws_store};
use crate::ui;

use super::shared::{list_worktrees, load_repo_workspaces};

/// Attempt to clean up a non-empty worktree directory and remove it.
///
/// This handles the "Directory not empty" error from git worktree remove by:
/// 1. Running `git clean -fd` inside the worktree to remove untracked files/dirs
/// 2. Retrying the worktree removal
/// 3. If that fails, attempting a force removal with `rm -rf`
/// 4. Running `git worktree prune` to clean up the git metadata
///
/// If all cleanup attempts fail, returns an error with helpful suggestions.
fn try_cleanup_and_remove(wt_path: &str, force: bool) -> Result<()> {
    let path = Path::new(wt_path);

    if !path.exists() {
        // Directory already gone, just prune
        gitcmd::git_output(&["worktree", "prune"]).ok();
        return Ok(());
    }

    ui::print_info("Directory not empty. Attempting cleanup...");

    // Step 1: Try to clean untracked files in the worktree
    let clean_result = Command::new(gitcmd::git_exe())
        .args(["-C", wt_path, "clean", "-fd"])
        .output();

    if let Ok(output) = clean_result {
        if output.status.success() {
            ui::print_info("Cleaned untracked files.");
        }
    }

    // Step 2: Retry the worktree removal
    let mut retry_args = vec!["worktree", "remove"];
    if force {
        retry_args.push("--force");
    }
    retry_args.push(wt_path);

    if gitcmd::git_output(&retry_args).is_ok() {
        return Ok(());
    }

    // Step 3: Still failing - check if we can identify what's blocking
    let blocking_items = find_blocking_items(wt_path);

    // Step 4: If force flag is set, try harder with rm -rf
    if force {
        ui::print_warning("Force removing directory...");

        // First, unregister the worktree from git
        let _ = gitcmd::git_output(&["worktree", "remove", "--force", wt_path]);

        // Then remove the directory manually
        if let Err(e) = std::fs::remove_dir_all(path) {
            bail!(
                "Failed to force remove directory '{}': {}\n\n\
                 Blocking items found:\n{}\n\n\
                 Try manually:\n  \
                 rm -rf '{}'\n  \
                 git worktree prune",
                wt_path,
                e,
                blocking_items,
                wt_path
            );
        }

        // Clean up git metadata
        gitcmd::git_output(&["worktree", "prune"]).ok();
        return Ok(());
    }

    // Step 5: Not forced and cleanup failed - provide helpful error
    bail!(
        "Cannot remove worktree at '{}': directory not empty.\n\n\
         Blocking items found:\n{}\n\n\
         Options:\n  \
         1. Use --force to remove anyway:\n     \
            {} workspace delete <name> --force\n\n  \
         2. Clean manually then retry:\n     \
            rm -rf '{}'\n     \
            git worktree prune",
        wt_path,
        blocking_items,
        crate::bin_name(),
        wt_path
    );
}

/// Find files/directories that may be blocking worktree removal.
fn find_blocking_items(wt_path: &str) -> String {
    let path = Path::new(wt_path);
    let mut items = Vec::new();

    // Check for common blocking patterns
    let blocking_patterns = [
        "node_modules",
        ".next",
        "target",
        "build",
        "dist",
        ".turbo",
        "__pycache__",
        ".pytest_cache",
        "vendor",
        ".gradle",
    ];

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip .git as it's expected
            if name == ".git" {
                continue;
            }

            let is_blocking = blocking_patterns.iter().any(|&p| name == p);
            let metadata = entry.metadata().ok();
            let is_dir = metadata.as_ref().is_some_and(|m| m.is_dir());

            if is_blocking {
                items.push(format!("  • {} (build artifact)", name));
            } else if is_dir {
                // Check if it's a non-empty directory
                if let Ok(mut sub) = std::fs::read_dir(entry.path()) {
                    if sub.next().is_some() {
                        items.push(format!("  • {}/ (directory)", name));
                    }
                }
            } else {
                items.push(format!("  • {}", name));
            }
        }
    }

    if items.is_empty() {
        "  (unable to identify specific items)".to_string()
    } else if items.len() > 10 {
        items.truncate(10);
        items.push("  ... and more".to_string());
        items.join("\n")
    } else {
        items.join("\n")
    }
}

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
            } else if msg.contains("Directory not empty") || msg.contains("not empty") {
                // Attempt to clean up the directory and retry
                match try_cleanup_and_remove(&wt_path, force) {
                    Ok(_) => {
                        // Successfully cleaned up and removed
                    }
                    Err(cleanup_err) => {
                        return Err(cleanup_err);
                    }
                }
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

    #[test]
    fn removes_worktree_with_untracked_files() {
        let repo = TestRepo::new();
        let ctx = repo.ctx();
        create::run(&ctx, "dirty-ws", None, None, None, false).unwrap();

        let (_, before) = load_repo_workspaces(&repo.conn).unwrap();
        let row = before
            .iter()
            .find(|w| w.name == "dirty-ws")
            .expect("setup row")
            .clone();
        let wt_path = Path::new(&row.path);

        // Create an untracked file that would cause "directory not empty"
        let untracked_file = wt_path.join("untracked.txt");
        std::fs::write(&untracked_file, "test content").unwrap();
        assert!(untracked_file.exists(), "untracked file should exist");

        // Delete with force should succeed
        run(&ctx, "dirty-ws", true).expect("delete with force should succeed");

        // The worktree directory is gone
        assert!(!wt_path.exists(), "worktree dir should be removed");
        // And the DB row is gone too
        let (_, after) = load_repo_workspaces(&repo.conn).unwrap();
        assert!(after.iter().all(|w| w.name != "dirty-ws"));
    }
}
