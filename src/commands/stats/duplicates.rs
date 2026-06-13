//! Special mode: `g stats --duplicates` — list repeated commit subjects so
//! you can spot copy-pasted messages and low-signal subjects.

use anyhow::Result;

use crate::commands::Ctx;
use crate::storage::stats as db;
use crate::ui;
use crate::ui::indent;

pub(super) fn run(ctx: &Ctx) -> Result<()> {
    let conn = ctx.conn;
    ui::print_blank();
    ui::print_fieldset("Duplicate Commit Messages");
    ui::print_blank();

    let duplicates = db::find_duplicate_commits(conn, None, 20)?;

    if duplicates.is_empty() {
        ui::print_info("No duplicate commit messages found.");
        ui::print_tip("Import git history first with: g stats --import");
        ui::print_blank();
        return Ok(());
    }

    let max_count = duplicates.iter().map(|(_, n)| *n).max().unwrap_or(1);

    for (subject, count) in &duplicates {
        let bar_len = ((*count as f64 / max_count as f64) * 20.0).round() as usize;
        let bar = format!(
            "{}{}",
            ui::danger(&"█".repeat(bar_len)),
            ui::muted(&"░".repeat(20 - bar_len))
        );

        let truncated_subject = if subject.len() > 50 {
            format!("{}...", &subject[..47])
        } else {
            subject.clone()
        };

        println!(
            "{}{}  {}  {}",
            indent(),
            ui::danger_bold(&format!("{:>3}x", count)),
            bar,
            ui::paint_text(&truncated_subject),
        );
    }

    ui::print_blank();
    ui::print_tip("Duplicate messages may indicate copy-paste commits or repetitive work");
    ui::print_blank();
    Ok(())
}
