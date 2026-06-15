//! `g workspace status` — print information about the current worktree
//! (name, branch, path, age, working-tree status summary), or emit a
//! machine-readable JSON document when `--json` is given.

use anyhow::Result;
use serde::Serialize;
use std::path::Path;

use crate::commands::git as gitcmd;
use crate::commands::Ctx;
use crate::ui;

use super::shared::{format_relative_time, list_worktrees, load_repo_workspaces};

// ─── JSON output shape ──────────────────────────────────────────────────────

#[derive(Serialize, Default)]
struct GitStatusJson {
    /// `true` when `git status --porcelain` produces no output.
    clean: bool,
    /// Total number of changed entries (staged + unstaged + untracked).
    changes: usize,
    staged: usize,
    unstaged: usize,
    untracked: usize,
}

#[derive(Serialize)]
struct CurrentWorkspaceJson<'a> {
    name: Option<&'a str>,
    branch: &'a str,
    path: String,
    description: Option<&'a str>,
    /// RFC 3339 UTC timestamp, or `null` when git-only.
    created_at: Option<String>,
    git_status: GitStatusJson,
}

// ─── run ────────────────────────────────────────────────────────────────────

pub(super) fn run(ctx: &Ctx, json: bool) -> Result<()> {
    let conn = ctx.conn;
    let (_, workspaces) = load_repo_workspaces(conn)?;
    let cwd = std::env::current_dir()?;
    let worktrees = list_worktrees()?;

    let current_wt = worktrees.iter().find(|wt| cwd.starts_with(&wt.path));

    // ── JSON output ─────────────────────────────────────────────────────────
    if json {
        let payload = current_wt.map(|wt| {
            let branch = wt.branch.as_deref().unwrap_or("(detached)");
            let meta = workspaces.iter().find(|ws| Path::new(&ws.path) == wt.path);
            CurrentWorkspaceJson {
                name: meta.map(|ws| ws.name.as_str()),
                branch,
                path: wt.path.to_string_lossy().into_owned(),
                description: meta.and_then(|ws| ws.description.as_deref()),
                created_at: meta.map(|ws| ws.created_at.to_rfc3339()),
                git_status: compute_git_status(),
            }
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

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

/// Build the `[git_status]` block for the JSON output by parsing
/// `git status --porcelain` once.
fn compute_git_status() -> GitStatusJson {
    let porcelain = gitcmd::git_output_lossy(&["status", "--porcelain"]);
    let lines: Vec<&str> = porcelain.lines().collect();
    let staged = lines
        .iter()
        .filter(|l| l.len() >= 2 && &l[0..1] != " " && &l[0..1] != "?")
        .count();
    let unstaged = lines
        .iter()
        .filter(|l| l.len() >= 2 && &l[1..2] != " " && &l[0..1] != "?")
        .count();
    let untracked = lines.iter().filter(|l| l.starts_with("??")).count();
    GitStatusJson {
        clean: lines.is_empty(),
        changes: lines.len(),
        staged,
        unstaged,
        untracked,
    }
}
