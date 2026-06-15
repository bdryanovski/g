//! `g workspace list` — render a table of every live worktree with its
//! optional metadata from the workspaces table, or emit machine-readable
//! JSON when `--json` is given.

use anyhow::Result;
use serde::Serialize;
use std::path::Path;

use crate::commands::Ctx;
use crate::ui;

use super::shared::{format_relative_time, list_worktrees, load_repo_workspaces};

// ─── JSON output shape ──────────────────────────────────────────────────────

/// One workspace in the JSON output.  `created_at` and `description` are
/// `null` for "git-only" worktrees that aren't tracked in the workspaces
/// table (typically the main repo checkout).
#[derive(Serialize)]
struct WorkspaceJson<'a> {
    name: &'a str,
    branch: &'a str,
    path: String,
    head: &'a str,
    is_current: bool,
    /// `null` when the row is git-only (no metadata in `workspaces`).
    description: Option<&'a str>,
    /// RFC 3339 UTC timestamp, or `null` when git-only.
    created_at: Option<String>,
}

// ─── run ────────────────────────────────────────────────────────────────────

pub(super) fn run(ctx: &Ctx, json: bool) -> Result<()> {
    let conn = ctx.conn;
    let (repo_id, workspaces) = load_repo_workspaces(conn)?;
    let _ = repo_id;
    let worktrees = list_worktrees()?;
    let cwd = std::env::current_dir()?;

    // ── JSON output ─────────────────────────────────────────────────────────
    if json {
        let payload: Vec<WorkspaceJson> = worktrees
            .iter()
            .filter(|wt| !wt.bare)
            .map(|wt| {
                let branch = wt.branch.as_deref().unwrap_or("(detached)");
                let is_current = cwd.starts_with(&wt.path);
                let meta = workspaces.iter().find(|ws| Path::new(&ws.path) == wt.path);
                let head: &str = if wt.head.len() >= 7 {
                    &wt.head[..7]
                } else {
                    &wt.head
                };
                WorkspaceJson {
                    name: meta.map(|ws| ws.name.as_str()).unwrap_or("(main)"),
                    branch,
                    path: wt.path.to_string_lossy().into_owned(),
                    head,
                    is_current,
                    description: meta.and_then(|ws| ws.description.as_deref()),
                    created_at: meta.map(|ws| ws.created_at.to_rfc3339()),
                }
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    if worktrees.is_empty() {
        ui::print_blank();
        ui::print_info("No worktrees found.");
        ui::print_blank();
        return Ok(());
    }

    ui::print_blank();
    ui::print_fieldset("Workspaces");
    ui::print_blank();
    let mut table = ui::Table::new(vec!["", "Name", "Branch", "Path", "HEAD", "Created"]);

    for wt in &worktrees {
        if wt.bare {
            continue;
        }

        let branch_display = wt.branch.as_deref().unwrap_or("(detached)");
        let is_current = cwd.starts_with(&wt.path);

        let head_display = ui::muted(if wt.head.len() >= 7 {
            &wt.head[..7]
        } else {
            &wt.head
        });

        let meta = workspaces.iter().find(|ws| Path::new(&ws.path) == wt.path);

        let (name_display, created_display) = if let Some(ws) = meta {
            let name = ui::branch_name_colored(&ws.name, is_current);
            let label = match &ws.description {
                Some(desc) if !desc.is_empty() => {
                    format!("{}  {}", name, ui::muted(desc))
                }
                _ => name,
            };
            (label, format_relative_time(ws.created_at))
        } else {
            let name = if is_current {
                ui::success_bold("(main)")
            } else {
                ui::muted("(main)")
            };
            (name, ui::muted("\u{2014}"))
        };

        let marker = ui::branch_marker(is_current);

        table.add_row(vec![
            marker,
            name_display,
            ui::color_branch(branch_display),
            ui::muted(&wt.path.display().to_string()),
            head_display,
            created_display,
        ]);
    }

    table.print();
    ui::print_blank();

    if workspaces.is_empty() {
        ui::print_tip(&format!(
            "{} workspace create <name>  to create a worktree workspace",
            crate::bin_name()
        ));
        ui::print_blank();
    }

    Ok(())
}
