//! Section: **Commit Types** — your `feat`/`fix`/`docs`/… distribution
//! recorded by `g commit`, each bar coloured by its conventional-commit type.

use anyhow::Result;

use crate::commands::Ctx;
use crate::storage::stats as db;
use crate::ui;
use crate::ui::indent;

pub(super) fn run(ctx: &Ctx) -> Result<()> {
    let conn = ctx.conn;
    let items = db::commit_type_counts(conn)?;
    if items.is_empty() {
        return Ok(());
    }

    ui::print_fieldset("Commit Types  (via g commit)");
    ui::print_blank();
    render_commit_type_chart(&items, 28);
    ui::print_blank();
    Ok(())
}

/// Like `shared::render_bar_chart` but colours each bar based on its
/// conventional-commit type.
fn render_commit_type_chart(items: &[(String, i64)], bar_width: usize) {
    let max_count = items.iter().map(|(_, n)| *n).max().unwrap_or(1).max(1);
    let max_label = items
        .iter()
        .map(|(l, _)| console::measure_text_width(l))
        .max()
        .unwrap_or(0);

    for (label, count) in items {
        let filled = (*count as usize * bar_width) / max_count as usize;
        let bar_color = commit_type_color(label);
        let filled_str = format!("{bar_color}{}{}\x1b[0m", "█".repeat(filled), "");
        let empty_str = ui::muted(&"░".repeat(bar_width - filled));
        let label_pad = " ".repeat(max_label - console::measure_text_width(label));
        println!(
            "{}{}{}  {}{}  {}",
            indent(),
            ui::paint_text(label),
            label_pad,
            filled_str,
            empty_str,
            ui::muted(&count.to_string()),
        );
    }
}

/// Return the ANSI color escape for a conventional-commit type.
fn commit_type_color(t: &str) -> &'static str {
    match t {
        "feat" => "\x1b[32m",                   // green
        "fix" => "\x1b[31m",                    // red
        "docs" => "\x1b[34m",                   // blue
        "refactor" => "\x1b[35m",               // magenta
        "perf" => "\x1b[36m",                   // cyan
        "test" => "\x1b[33m",                   // yellow
        "chore" | "build" | "ci" => "\x1b[90m", // dark gray
        "revert" => "\x1b[2;31m",               // dim red
        _ => "\x1b[37m",                        // default white
    }
}
