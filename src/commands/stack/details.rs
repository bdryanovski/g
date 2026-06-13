//! `g stack details` — current stack with per-branch commits and live PR status.

use anyhow::Result;

use crate::commands::git as gitcmd;
use crate::commands::Ctx;
use crate::config;
use crate::github;
use crate::ui;

use super::shared::{current_stack, get_github_token};

/// Show the current stack with per-branch commit details and live PR status.
pub(super) fn run(ctx: &Ctx) -> Result<()> {
    let conn = ctx.conn;
    let stack = current_stack(conn)?;
    let current_branch = gitcmd::current_branch().unwrap_or_default();

    let open_prs = fetch_open_prs();

    ui::print_blank();
    ui::print_fieldset(&format!(
        "Stack: {}  root: {}",
        stack.name, stack.root_branch
    ));
    ui::print_blank();

    let last_branch = stack.branches.len().saturating_sub(1);

    for (i, branch) in stack.branches.iter().enumerate() {
        let is_current = branch.name == current_branch;
        let connector = if i == last_branch {
            "\u{2514}\u{2500}\u{2500}"
        } else {
            "\u{251c}\u{2500}\u{2500}"
        };
        let pipe = if i == last_branch { " " } else { "\u{2502}" };

        let marker = ui::branch_marker(is_current);
        let name_colored = ui::branch_name_colored(&branch.name, is_current);

        print!("  {} {} {}", ui::muted(connector), marker, name_colored);
        if is_current {
            print!("  {}", ui::dimmed("(current)"));
        }
        ui::print_blank();

        let branch_time = gitcmd::git_output_lossy(&["log", "-1", "--format=%ar", &branch.name]);
        if !branch_time.is_empty() {
            ui::print_indented(&format!(
                "{}     {}",
                ui::muted(pipe),
                ui::muted(branch_time.trim())
            ));
        }

        let live_pr = open_prs.as_ref().and_then(|prs| prs.get(&branch.name));
        if let Some(pr) = live_pr {
            ui::print_indented(&format!(
                "{}     {} {}  {}",
                ui::muted(pipe),
                ui::muted("PR"),
                ui::primary(&format!("#{}", pr.number)),
                ui::link_muted(&pr.html_url)
            ));
        } else if let Some(pr_url) = &branch.pr_url {
            let pr_num = branch
                .pr_number
                .map(|n| format!("#{}", n))
                .unwrap_or_default();
            ui::print_indented(&format!(
                "{}     {} {}  {}",
                ui::muted(pipe),
                ui::muted("PR"),
                ui::primary(&pr_num),
                ui::link_muted(pr_url)
            ));
        }

        let base = if i == 0 {
            &stack.root_branch
        } else {
            &stack.branches[i - 1].name
        };

        if i > 0 || branch.name != stack.root_branch {
            let range = format!("{}..{}", base, branch.name);
            let commits = gitcmd::git_output_lossy(&[
                "log",
                "--format=%h%x1f%s%x1f%an%x1f%ar",
                "--reverse",
                &range,
            ]);

            if !commits.is_empty() {
                ui::print_indented(&ui::muted(pipe));
                for commit_line in commits.lines() {
                    let parts: Vec<&str> = commit_line.split('\x1f').collect();
                    if parts.len() >= 4 {
                        ui::print_indented(&format!(
                            "{}     {} - {}  {}",
                            ui::muted(pipe),
                            ui::color_hash(parts[0]),
                            ui::muted(parts[1]),
                            ui::dimmed(&format!("({}, {})", parts[2], parts[3]))
                        ));
                    } else if let Some((hash, subject)) = commit_line.split_once(' ') {
                        ui::print_indented(&format!(
                            "{}     {} - {}",
                            ui::muted(pipe),
                            ui::color_hash(hash),
                            ui::muted(subject)
                        ));
                    }
                }
            } else {
                ui::print_indented(&format!(
                    "{}     {}",
                    ui::muted(pipe),
                    ui::muted("(no commits)")
                ));
            }
        }

        if i < last_branch {
            ui::print_indented(&format!(
                "{}   {}",
                ui::muted("\u{2502}"),
                ui::muted("\u{2502}")
            ));
        }
    }

    ui::print_blank();
    Ok(())
}

/// Try to fetch all open PRs from GitHub for the inline display.
///
/// Returns `None` silently when no token is configured or the API call fails.
fn fetch_open_prs() -> Option<std::collections::HashMap<String, github::PrInfo>> {
    let cfg = config::load().ok()?;
    let token = get_github_token(&cfg).ok()?;
    let (owner, repo_name) = github::detect_repo().ok()?;
    github::list_open_prs(&token, &cfg.github.api_base, &owner, &repo_name).ok()
}
