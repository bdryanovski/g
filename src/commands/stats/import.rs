//! Special mode: `g stats --import` — backfill git history into the local
//! database so commit-message features (search, duplicates, length stats) work.

use anyhow::Result;

use crate::commands::git as git_cmd;
use crate::commands::Ctx;
use crate::storage::{repos, stats as db};
use crate::ui;

use super::shared::fmt_n;

pub(super) fn run(ctx: &Ctx, limit: Option<usize>) -> Result<()> {
    let conn = ctx.conn;
    ui::print_blank();
    ui::print_fieldset("Import Git History");
    ui::print_blank();

    let repo_root = match git_cmd::repo_root() {
        Ok(r) => r,
        Err(_) => {
            ui::print_warning("Not inside a git repository.");
            return Ok(());
        }
    };

    let repo_id = repos::upsert(conn, &repo_root)?;

    let pb = ui::spinner("Importing commits...");
    let count = db::import_git_history(conn, repo_id, limit)?;

    if count > 0 {
        ui::spinner_success(pb, &format!("Imported {} commits", fmt_n(count as i64)));
    } else {
        ui::spinner_success(pb, "No new commits to import");
    }

    let total = db::total_commit_messages(conn, Some(repo_id))?;
    ui::print_key_value_pairs(&[("Total commits in database", ui::paint_text(&fmt_n(total)))]);

    ui::print_blank();
    Ok(())
}
