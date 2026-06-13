//! Section: **Repository Activity** — which repos you commit in most often.

use anyhow::Result;

use crate::commands::Ctx;
use crate::storage::stats as db;
use crate::ui;

use super::shared::render_bar_chart;

pub(super) fn run(ctx: &Ctx) -> Result<()> {
    let conn = ctx.conn;
    let items = db::top_repos_by_activity(conn, 10)?;
    if items.is_empty() {
        return Ok(());
    }

    ui::print_fieldset("Repository Activity");
    ui::print_blank();
    render_bar_chart(&items, 28);
    ui::print_blank();
    Ok(())
}
