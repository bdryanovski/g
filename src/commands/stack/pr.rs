//! `g stack pr` — create or update GitHub PRs so each PR targets the branch
//! below it in the stack.

use anyhow::Result;

use crate::commands::git as gitcmd;
use crate::commands::Ctx;
use crate::config;
use crate::github;
use crate::storage::stacks as stacks_store;
use crate::ui;

use super::shared::{current_stack, get_github_token, open_url};

/// Create or update GitHub PRs for every non-root branch in the current stack.
pub(super) fn run(ctx: &Ctx, open: bool, draft: bool) -> Result<()> {
    let conn = ctx.conn;
    let stack = current_stack(conn)?;
    let cfg = config::load()?;

    let (owner, repo_name) = github::detect_repo()?;

    ui::print_blank();
    ui::print_fieldset(&format!(
        "Creating PRs: {}  \u{2192}  {}/{}",
        stack.name, owner, repo_name
    ));
    ui::print_blank();

    if gitcmd::is_dry_run() {
        for i in 1..stack.branches.len() {
            let base = stack.branches[i - 1].name.clone();
            let branch = stack.branches[i].name.clone();
            let has_pr = stack.branches[i].pr_number.is_some();

            if has_pr {
                gitcmd::dry_run_action(
                    &format!("GitHub API: check/update PR for '{}'", branch),
                    &format!(
                        "Verify existing PR for '{}' \u{2192} '{}' has correct base, update if needed",
                        branch, base
                    ),
                );
            } else {
                let draft_note = if draft { " as draft" } else { "" };
                gitcmd::dry_run_action(
                    &format!("GitHub API: create PR '{}' \u{2192} '{}'", branch, base),
                    &format!(
                        "Open a new pull request{} from '{}' into '{}' on {}/{}",
                        draft_note, branch, base, owner, repo_name
                    ),
                );
            }
        }
        gitcmd::dry_run_action("Save PR metadata", "Update g.db with PR numbers and URLs");
        return Ok(());
    }

    let token = get_github_token(&cfg)?;

    for i in 1..stack.branches.len() {
        let base = stack.branches[i - 1].name.clone();
        let branch = stack.branches[i].name.clone();

        let pb = ui::spinner(&format!(
            "Creating PR: {} \u{2192} {}",
            ui::success(&branch),
            ui::primary(&base)
        ));

        let existing = github::find_pr(&token, &cfg.github.api_base, &owner, &repo_name, &branch)?;

        let result: Result<github::PrInfo> = if let Some(pr) = existing {
            if pr.base_ref != base {
                pb.set_message(format!(
                    "Updating PR #{} base: {} \u{2192} {}",
                    pr.number,
                    ui::danger(&pr.base_ref),
                    ui::success(&base)
                ));
                let updated = github::update_pr_base(
                    &token,
                    &cfg.github.api_base,
                    &owner,
                    &repo_name,
                    pr.number,
                    &base,
                )?;
                pb.finish_and_clear();
                Ok(updated)
            } else {
                pb.finish_and_clear();
                Ok(pr)
            }
        } else {
            let pr_title = gitcmd::git_output_lossy(&["log", "--format=%s", "-1", &branch]);
            let title = if pr_title.is_empty() {
                branch.clone()
            } else {
                pr_title
            };
            let pr = github::create_pr(
                &token,
                &cfg.github.api_base,
                &owner,
                &repo_name,
                &title,
                &branch,
                &base,
                draft,
            )?;
            pb.finish_and_clear();
            Ok(pr)
        };

        match result {
            Ok(pr) => {
                let action = if stack.branches[i].pr_number.is_some() {
                    "Updated"
                } else {
                    "Created"
                };
                ui::print_success(&format!(
                    "{} PR #{}: {} \u{2192} {}  {}",
                    action,
                    ui::warning(&pr.number.to_string()),
                    ui::success_bold(&branch),
                    ui::primary(&base),
                    ui::link_muted(&pr.html_url)
                ));

                stacks_store::update_branch_pr(conn, stack.id, &branch, pr.number, &pr.html_url)
                    .ok(); // best-effort

                if open {
                    let _ = open_url(&pr.html_url);
                }
            }
            Err(e) => {
                ui::print_error(&format!(
                    "Failed to create PR for {}: {}",
                    ui::danger(&branch),
                    e
                ));
            }
        }
    }

    ui::print_blank();
    Ok(())
}
