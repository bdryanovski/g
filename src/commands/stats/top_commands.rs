//! Section: **Top Commands** — the `g`/git subcommands you run most often.

use anyhow::Result;

use crate::commands::Ctx;
use crate::storage::stats as db;
use crate::ui;

use super::shared::render_bar_chart;

pub(super) fn run(ctx: &Ctx) -> Result<()> {
    let conn = ctx.conn;
    let items = db::top_commands(conn, 12)?;

    // "git" is recorded for every passthrough command, so it trivially
    // dominates the chart while providing no meaningful insight — the whole
    // tool is built on top of git.  Drop it before rendering.
    let items: Vec<(String, i64)> = items
        .into_iter()
        .filter(|(name, _)| name != "git")
        .collect();

    if items.is_empty() {
        return Ok(());
    }

    ui::print_fieldset("Top Commands");
    ui::print_blank();
    render_bar_chart(&items, 28);
    ui::print_blank();
    Ok(())
}
