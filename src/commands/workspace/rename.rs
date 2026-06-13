//! `g workspace rename <old> <new>` — move a workspace directory and repair
//! the git worktree tracking afterwards.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::commands::git as gitcmd;
use crate::commands::Ctx;
use crate::storage::{stats, workspaces as ws_store};
use crate::ui;

use super::shared::{load_repo_workspaces, worktree_path_for};

pub(super) fn run(ctx: &Ctx, old: &str, new: &str) -> Result<()> {
    let conn = ctx.conn;
    let (_, workspaces) = load_repo_workspaces(conn)?;

    let ws = workspaces
        .iter()
        .find(|w| w.name == old)
        .or_else(|| workspaces.iter().find(|w| w.branch == old))
        .with_context(|| format!("Workspace '{}' not found.", old))?;

    let ws_id = ws.id;
    let old_path = PathBuf::from(&ws.path);
    let new_path = worktree_path_for(conn, new)?;

    if new_path.exists() {
        bail!(
            "Directory '{}' already exists. Choose a different name.",
            new_path.display()
        );
    }

    if !old_path.exists() {
        bail!(
            "Workspace directory '{}' no longer exists. Clean up with `{} workspace delete {}`.",
            old_path.display(),
            crate::bin_name(),
            old
        );
    }

    if !gitcmd::is_dry_run() {
        let pb = ui::spinner(&format!("Moving worktree {} → {}", old, new));

        fs::rename(&old_path, &new_path).with_context(|| {
            format!(
                "Failed to move '{}' to '{}'",
                old_path.display(),
                new_path.display()
            )
        })?;

        gitcmd::git_output(&["worktree", "repair"])
            .context("Failed to repair worktree tracking after move")?;

        ws_store::update_name_and_path(conn, ws_id, new, &new_path.to_string_lossy())?;
        stats::record_workspace_event(conn, Some(ws_id), None, "rename").ok();

        ui::spinner_success(
            pb,
            &format!(
                "Renamed workspace '{}' \u{2192} '{}'",
                old,
                ui::success(new)
            ),
        );
        ui::print_indented(&format!(
            "{} {}",
            ui::muted("path:"),
            ui::link_primary_bold(&new_path.display().to_string())
        ));
        ui::print_blank();
    } else {
        gitcmd::dry_run_action(
            &format!("mv {} {}", old_path.display(), new_path.display()),
            &format!(
                "Move worktree directory from '{}' to '{}'",
                old_path.display(),
                new_path.display()
            ),
        );
        gitcmd::git_mutate(
            &["worktree", "repair"],
            "Repair git worktree tracking after the directory move",
        )?;
        gitcmd::dry_run_action(
            "Update workspace metadata",
            &format!("Rename workspace '{}' → '{}' in g.db", old, new),
        );
    }

    Ok(())
}
