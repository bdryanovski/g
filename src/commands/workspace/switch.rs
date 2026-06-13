//! `g workspace switch [name]` — open an interactive subshell inside a
//! workspace directory.  With no `name` argument, a fuzzy picker is shown.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::commands::Ctx;
use crate::storage::stats;
use crate::ui;

use super::shared::{list_worktrees, load_repo_workspaces, resolve_workspace, ResolvedWorkspace};

pub(super) fn run(ctx: &Ctx, name: Option<&str>) -> Result<()> {
    let conn = ctx.conn;
    let (_, workspaces) = load_repo_workspaces(conn)?;
    let worktrees = list_worktrees()?;
    let cwd = std::env::current_dir()?;

    // Resolve target — either a DB row or a git-only worktree.
    let resolved: ResolvedWorkspace<'_> = match name {
        Some(n) => resolve_workspace(&workspaces, n)?,

        None => {
            // Build a candidate list from *all* live worktrees, not just those
            // with DB records.
            struct PickerEntry {
                display_name: String,
                branch: String,
                path: PathBuf,
            }

            let candidates: Vec<PickerEntry> = worktrees
                .iter()
                .filter(|wt| !wt.bare)
                .map(|wt| {
                    let meta = workspaces.iter().find(|ws| Path::new(&ws.path) == wt.path);
                    let branch = wt.branch.as_deref().unwrap_or("(detached)").to_string();
                    match meta {
                        Some(ws) => PickerEntry {
                            display_name: ws.name.clone(),
                            branch: ws.branch.clone(),
                            path: wt.path.clone(),
                        },
                        None => PickerEntry {
                            display_name: branch.clone(),
                            branch,
                            path: wt.path.clone(),
                        },
                    }
                })
                .collect();

            if candidates.is_empty() {
                ui::print_blank();
                ui::print_indented(&ui::muted(
                    "No workspaces found. Use `g workspace create <name>` to create one.",
                ));
                ui::print_blank();
                return Ok(());
            }

            let name_width = candidates
                .iter()
                .map(|c| c.display_name.len())
                .max()
                .unwrap_or(0);
            let branch_width = candidates.iter().map(|c| c.branch.len()).max().unwrap_or(0);

            let items: Vec<String> = candidates
                .iter()
                .map(|c| {
                    let marker = if cwd.starts_with(&c.path) {
                        "\u{25c9}"
                    } else {
                        "\u{25ef}"
                    };
                    format!(
                        "{} {:<name_w$}  {:<branch_w$}  {}",
                        marker,
                        c.display_name,
                        c.branch,
                        c.path.display(),
                        name_w = name_width,
                        branch_w = branch_width,
                    )
                })
                .collect();

            let item_strs: Vec<&str> = items.iter().map(String::as_str).collect();
            let selection = ui::fuzzy_select("Switch to workspace", &item_strs);

            match selection {
                None => {
                    ui::print_blank();
                    ui::print_info("Cancelled.");
                    ui::print_blank();
                    return Ok(());
                }
                Some(idx) => {
                    let picked = &candidates[idx];
                    // Try to return the DB row if available, otherwise GitOnly.
                    if let Some(ws) = workspaces
                        .iter()
                        .find(|ws| Path::new(&ws.path) == picked.path)
                    {
                        ResolvedWorkspace::DbRow(ws)
                    } else {
                        ResolvedWorkspace::GitOnly {
                            path: picked.path.clone(),
                            branch: picked.branch.clone(),
                        }
                    }
                }
            }
        }
    };

    // Extract path and display info from the resolved workspace.
    let (wt_path, display_name, branch, ws_id, repo_id, description) = match &resolved {
        ResolvedWorkspace::DbRow(ws) => (
            PathBuf::from(&ws.path),
            ws.name.clone(),
            ws.branch.clone(),
            Some(ws.id),
            Some(ws.repo_id),
            ws.description.clone(),
        ),
        ResolvedWorkspace::GitOnly { path, branch } => (
            path.clone(),
            branch.clone(),
            branch.clone(),
            None,
            None,
            None,
        ),
    };

    if !wt_path.exists() {
        bail!(
            "Workspace directory '{}' no longer exists. It may have been removed outside {}.\n\
             Run `{} workspace delete {}` to clean up.",
            wt_path.display(),
            crate::bin_name(),
            crate::bin_name(),
            display_name
        );
    }

    stats::record_workspace_event(conn, ws_id, repo_id, "switch").ok();

    ui::print_blank();
    ui::print_info(&format!(
        "Opening shell in workspace {} \u{2192} {}",
        ui::success_bold(&display_name),
        ui::link_primary_bold(&wt_path.to_string_lossy())
    ));
    let mut pairs: Vec<(&str, String)> = vec![("branch", ui::success_bold(&branch))];
    if let Some(desc) = &description {
        if !desc.is_empty() {
            pairs.insert(0, ("desc", ui::paint_text(desc)));
        }
    }
    pairs.push(("hint", ui::muted("Ctrl+D or `exit` to return")));
    ui::print_key_value_pairs(&pairs);
    ui::print_blank();

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());

    let status = Command::new(&shell)
        .current_dir(&wt_path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("Failed to spawn shell '{}'", shell))?;

    if !status.success() {
        if let Some(code) = status.code() {
            if code != 0 {
                ui::print_blank();
                ui::print_info(&format!("Shell exited with code {}", code));
            }
        }
    }

    ui::print_blank();
    ui::print_info("Returned to original directory.");
    ui::print_blank();

    Ok(())
}
