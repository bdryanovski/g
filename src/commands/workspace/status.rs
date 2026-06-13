//! `g workspace status` — print information about the current worktree
//! (name, branch, path, age, working-tree status summary).

use anyhow::Result;
use std::path::Path;

use crate::commands::git as gitcmd;
use crate::commands::Ctx;
use crate::ui;

use super::shared::{format_relative_time, list_worktrees, load_repo_workspaces};

pub(super) fn run(ctx: &Ctx) -> Result<()> {
    let conn = ctx.conn;
    let (_, workspaces) = load_repo_workspaces(conn)?;
    let cwd = std::env::current_dir()?;
    let worktrees = list_worktrees()?;

    let current_wt = worktrees.iter().find(|wt| cwd.starts_with(&wt.path));

    ui::print_blank();

    if let Some(wt) = current_wt {
        let branch = wt.branch.as_deref().unwrap_or("(detached)");
        let meta = workspaces.iter().find(|ws| Path::new(&ws.path) == wt.path);

        ui::print_fieldset("Current Workspace");
        ui::print_blank();

        if let Some(ws) = meta {
            let mut pairs: Vec<(&str, String)> = vec![
                (
                    "Workspace",
                    format!(
                        "{} {}",
                        ui::success_bold(&ws.name),
                        ui::muted(&format!("({})", ws.branch))
                    ),
                ),
                ("Path", ui::link_primary_bold(&ws.path)),
                ("Created", format_relative_time(ws.created_at)),
            ];
            if let Some(desc) = &ws.description {
                pairs.insert(1, ("Description", desc.to_string()));
            }
            ui::print_key_value_pairs(&pairs);
        } else {
            ui::print_key_value_pairs(&[("Worktree", ui::success_bold("(main repository)"))]);
        }

        ui::print_key_value_pairs(&[("Branch", ui::primary_bold(branch))]);

        let porcelain = gitcmd::git_output_lossy(&["status", "--porcelain"]);
        let changes: Vec<&str> = porcelain.lines().collect();
        if changes.is_empty() {
            ui::print_key_value_pairs(&[("Status", ui::success("clean"))]);
        } else {
            let staged = changes
                .iter()
                .filter(|l| l.len() >= 2 && &l[0..1] != " " && &l[0..1] != "?")
                .count();
            let unstaged = changes
                .iter()
                .filter(|l| l.len() >= 2 && &l[1..2] != " " && &l[0..1] != "?")
                .count();
            let untracked = changes.iter().filter(|l| l.starts_with("??")).count();
            let status_val = format!(
                "{} change{}{}{}",
                ui::warning(&changes.len().to_string()),
                if changes.len() == 1 { "" } else { "s" },
                if staged > 0 {
                    ui::success(&format!(" ({} staged)", staged))
                } else {
                    String::new()
                },
                if untracked > 0 {
                    ui::muted(&format!(" ({} untracked)", untracked))
                } else if unstaged > 0 {
                    ui::warning(&format!(" ({} unstaged)", unstaged))
                } else {
                    String::new()
                },
            );
            ui::print_key_value_pairs(&[("Status", status_val)]);
        }
    } else {
        ui::print_info("Not inside any known worktree.");
    }

    ui::print_blank();
    Ok(())
}
