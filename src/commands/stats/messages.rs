//! `--message-stats` mode sections: commit message length statistics, length
//! trends over time, top authors, and a top-N duplicate summary.

use anyhow::Result;

use crate::commands::Ctx;
use crate::storage::stats as db;
use crate::ui;
use crate::ui::indent;

use super::shared::{fmt_n, render_bar_chart};

// ─── Length stats ────────────────────────────────────────────────────────────

/// Overall commit message length statistics.
pub(super) fn length_stats(ctx: &Ctx) -> Result<()> {
    let conn = ctx.conn;
    let stats = db::commit_length_stats(conn, None)?;

    if stats.total_commits == 0 {
        return Ok(());
    }

    ui::print_fieldset("Commit Message Statistics");
    ui::print_blank();

    let long_pct = (stats.long_subjects as f64 / stats.total_commits as f64) * 100.0;

    ui::print_key_value_pairs(&[
        (
            "Total commits analyzed",
            ui::paint_text(&fmt_n(stats.total_commits)),
        ),
        (
            "Avg subject length",
            ui::paint_text(&format!("{:.1} chars", stats.avg_subject_length)),
        ),
        (
            "Avg body length",
            ui::paint_text(&format!("{:.1} chars", stats.avg_body_length)),
        ),
        (
            "Commits with body",
            ui::paint_text(&format!("{:.1}%", stats.body_percentage)),
        ),
        (
            "Long subjects (>72)",
            if long_pct > 20.0 {
                ui::danger(&format!("{} ({:.1}%)", stats.long_subjects, long_pct))
            } else {
                ui::success(&format!("{} ({:.1}%)", stats.long_subjects, long_pct))
            },
        ),
    ]);

    ui::print_blank();
    Ok(())
}

// ─── Length trends ───────────────────────────────────────────────────────────

/// Commit message length trends over the last 12 months.
pub(super) fn length_trends(ctx: &Ctx) -> Result<()> {
    let conn = ctx.conn;
    let trends = db::commit_length_over_time(conn, None, 12)?;

    if trends.is_empty() {
        return Ok(());
    }

    ui::print_fieldset("Subject Length Over Time (12 months)");
    ui::print_blank();

    let max_len = trends
        .iter()
        .map(|t| t.avg_subject_length)
        .fold(0.0_f64, f64::max);

    for trend in &trends {
        let bar_len = ((trend.avg_subject_length / max_len.max(1.0)) * 30.0).round() as usize;
        let bar_color = if trend.avg_subject_length > 72.0 {
            ui::danger(&"█".repeat(bar_len))
        } else if trend.avg_subject_length > 50.0 {
            ui::warning(&"█".repeat(bar_len))
        } else {
            ui::success(&"█".repeat(bar_len))
        };

        println!(
            "{}{}  {}{}  {} chars ({} commits)",
            indent(),
            ui::muted(&trend.month),
            bar_color,
            ui::muted(&"░".repeat(30 - bar_len)),
            ui::paint_text(&format!("{:.0}", trend.avg_subject_length)),
            trend.commit_count,
        );
    }

    ui::print_blank();
    Ok(())
}

// ─── Top authors ─────────────────────────────────────────────────────────────

/// Top authors by commit count.
pub(super) fn top_authors(ctx: &Ctx) -> Result<()> {
    let conn = ctx.conn;
    let authors = db::top_authors(conn, None, 10)?;

    if authors.is_empty() {
        return Ok(());
    }

    ui::print_fieldset("Top Authors");
    ui::print_blank();
    render_bar_chart(&authors, 28);
    ui::print_blank();
    Ok(())
}

// ─── Top duplicates summary ──────────────────────────────────────────────────

/// Top duplicate commits — short summary printed inside the full report; see
/// [`super::duplicates`] for the full standalone listing.
pub(super) fn top_duplicates(ctx: &Ctx) -> Result<()> {
    let conn = ctx.conn;
    let duplicates = db::find_duplicate_commits(conn, None, 5)?;

    if duplicates.is_empty() {
        return Ok(());
    }

    ui::print_fieldset("Top Duplicate Commit Messages");
    ui::print_blank();

    for (subject, count) in &duplicates {
        let truncated = if subject.len() > 50 {
            format!("{}...", &subject[..47])
        } else {
            subject.clone()
        };
        println!(
            "{}{}  {}",
            indent(),
            ui::danger_bold(&format!("{:>3}x", count)),
            ui::paint_text(&truncated),
        );
    }

    ui::print_blank();
    ui::print_tip("Run 'g stats --duplicates' for full list");
    ui::print_blank();
    Ok(())
}
