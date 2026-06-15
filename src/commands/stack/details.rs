//! `g stack details` — current stack with per-branch commits and live PR status.

use anyhow::Result;
use serde::Serialize;

use crate::commands::git as gitcmd;
use crate::commands::Ctx;
use crate::config;
use crate::github;
use crate::ui;

use super::shared::{current_stack, get_github_token};

// ─── JSON output shape ──────────────────────────────────────────────────────

/// One branch in the JSON output, with the per-branch commits the rendered
/// view also shows.
#[derive(Serialize)]
struct BranchDetails<'a> {
    name: &'a str,
    position: i32,
    is_current: bool,
    pr_number: Option<u64>,
    pr_url: Option<String>,
    /// Commits between this branch and its base — `[]` for the root branch.
    commits: Vec<CommitJson>,
}

#[derive(Serialize)]
struct CommitJson {
    hash: String,
    subject: String,
    author: String,
    relative_date: String,
}

#[derive(Serialize)]
struct StackDetailsJson<'a> {
    name: &'a str,
    root_branch: &'a str,
    branches: Vec<BranchDetails<'a>>,
}

// ─── run ────────────────────────────────────────────────────────────────────

/// Show the current stack with per-branch commit details and live PR status.
pub(super) fn run(ctx: &Ctx, json: bool) -> Result<()> {
    let conn = ctx.conn;
    let stack = current_stack(conn)?;
    let current_branch = gitcmd::current_branch().unwrap_or_default();

    let open_prs = fetch_open_prs();

    if json {
        let branches: Vec<BranchDetails> = stack
            .branches
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let live_pr = open_prs.as_ref().and_then(|prs| prs.get(&b.name));
                let (pr_number, pr_url) = match live_pr {
                    Some(pr) => (Some(pr.number), Some(pr.html_url.clone())),
                    None => (b.pr_number, b.pr_url.clone()),
                };

                let base = if i == 0 {
                    &stack.root_branch
                } else {
                    &stack.branches[i - 1].name
                };
                let commits = if i == 0 && b.name == stack.root_branch {
                    Vec::new()
                } else {
                    let range = format!("{}..{}", base, b.name);
                    let raw = gitcmd::git_output_lossy(&[
                        "log",
                        "--format=%h%x1f%s%x1f%an%x1f%ar",
                        "--reverse",
                        &range,
                    ]);
                    raw.lines()
                        .filter_map(|line| {
                            let parts: Vec<&str> = line.split('\x1f').collect();
                            (parts.len() >= 4).then(|| CommitJson {
                                hash: parts[0].to_string(),
                                subject: parts[1].to_string(),
                                author: parts[2].to_string(),
                                relative_date: parts[3].to_string(),
                            })
                        })
                        .collect()
                };

                BranchDetails {
                    name: &b.name,
                    position: b.position,
                    is_current: b.name == current_branch,
                    pr_number,
                    pr_url,
                    commits,
                }
            })
            .collect();

        let payload = StackDetailsJson {
            name: &stack.name,
            root_branch: &stack.root_branch,
            branches,
        };
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

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
