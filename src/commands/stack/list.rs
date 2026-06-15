//! `g stack list` (and its `view` alias) — print every stack as a tree, or
//! emit a machine-readable JSON document when `--json` is given.

use serde::Serialize;

use crate::commands::prelude::*;
use crate::storage::stacks as stacks_store;

use super::shared::current_repo_id;

// ─── JSON output shape ──────────────────────────────────────────────────────

/// One branch entry inside a [`StackJson`].
#[derive(Serialize)]
struct BranchJson<'a> {
    name: &'a str,
    position: i32,
    is_current: bool,
    pr_number: Option<u64>,
    pr_url: Option<&'a str>,
}

/// One stack in the JSON output.
#[derive(Serialize)]
struct StackJson<'a> {
    name: &'a str,
    root_branch: &'a str,
    branches: Vec<BranchJson<'a>>,
}

// ─── run ────────────────────────────────────────────────────────────────────

/// List all stacks in the current repository.
pub(super) fn run(ctx: &Ctx, json: bool) -> Result<()> {
    let conn = ctx.conn;
    let repo_id = current_repo_id(conn)?;
    let stacks = stacks_store::load_all(conn, repo_id)?;
    let current_branch = gitcmd::current_branch().unwrap_or_default();

    // ── JSON output ─────────────────────────────────────────────────────────
    //
    // Always emit a top-level array (possibly empty) so consumers can parse
    // unconditionally with `jq '.[]'` etc.
    if json {
        let payload: Vec<StackJson> = stacks
            .iter()
            .map(|s| StackJson {
                name: &s.name,
                root_branch: &s.root_branch,
                branches: s
                    .branches
                    .iter()
                    .map(|b| BranchJson {
                        name: &b.name,
                        position: b.position,
                        is_current: b.name == current_branch,
                        pr_number: b.pr_number,
                        pr_url: b.pr_url.as_deref(),
                    })
                    .collect(),
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    if stacks.is_empty() {
        ui::print_blank();
        ui::print_info("No stacks yet.");
        ui::print_tip(&format!(
            "{} stack new <name>  to create a stack from the current branch",
            crate::bin_name()
        ));
        ui::print_blank();
        return Ok(());
    }

    for stack in &stacks {
        ui::print_blank();
        ui::print_fieldset(&format!(
            "Stack: {}  root: {}",
            stack.name, stack.root_branch
        ));
        ui::print_blank();

        let last = stack.branches.len().saturating_sub(1);
        for (i, branch) in stack.branches.iter().enumerate() {
            let is_current = branch.name == current_branch;
            let connector = if i == last {
                "  \u{2514}\u{2500}\u{2500}"
            } else {
                "  \u{251c}\u{2500}\u{2500}"
            };
            let marker = ui::branch_marker(is_current);
            let name_colored = ui::branch_name_colored(&branch.name, is_current);

            print!("{} {} {}", ui::muted(connector), marker, name_colored);

            if let Some(pr_url) = &branch.pr_url {
                let pr_num = branch
                    .pr_number
                    .map(|n| format!(" #{}", n))
                    .unwrap_or_default();
                print!("  {}{}", ui::muted("PR"), ui::primary(&pr_num));
                print!("  {}", ui::link_muted(pr_url));
            }

            if is_current {
                print!("  {}", ui::muted("\u{2190} you are here"));
            }
            ui::print_blank();

            if i < last {
                ui::print_indented(&format!(
                    "{}   {}",
                    ui::muted("\u{2502}"),
                    ui::muted("\u{2502}")
                ));
            }
        }
    }
    ui::print_blank();
    Ok(())
}

/// `g stack view` — alias for [`run`] (the tree view, never JSON).
pub(super) fn view(ctx: &Ctx) -> Result<()> {
    run(ctx, false)
}
