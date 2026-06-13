//! Section: **Usage Overview** — totals + streak information.

use anyhow::Result;

use crate::commands::Ctx;
use crate::storage::stats as db;
use crate::ui;

use super::shared::{fmt_duration_ms, fmt_n};

pub(super) fn run(ctx: &Ctx) -> Result<()> {
    let conn = ctx.conn;
    let s = db::query_overall(conn)?;
    let (current_streak, longest_streak) = db::streak_info(conn).unwrap_or((0, 0));

    ui::print_fieldset("Usage Overview");
    ui::print_blank();

    let error_rate = if s.total_commands > 0 {
        format!(
            "{:.1}%",
            s.total_errors as f64 / s.total_commands as f64 * 100.0
        )
    } else {
        "—".to_string()
    };

    let streak_label = |n: u32| -> String {
        if n == 0 {
            ui::muted("—")
        } else if n == 1 {
            ui::success("1 day")
        } else {
            ui::success(&format!("{} days", n))
        }
    };

    ui::print_key_value_pairs(&[
        ("Commands run", ui::primary_bold(&fmt_n(s.total_commands))),
        (
            "Commits (g commit)",
            ui::paint_text(&fmt_n(s.total_commits_recorded)),
        ),
        ("Repositories", ui::paint_text(&fmt_n(s.total_repos))),
        ("Active days", ui::paint_text(&fmt_n(s.active_days))),
        ("Current streak", streak_label(current_streak)),
        ("Longest streak", streak_label(longest_streak)),
        (
            "Avg cmd time",
            ui::muted(&fmt_duration_ms(s.avg_duration_ms)),
        ),
        ("Error rate", ui::muted(&error_rate)),
    ]);

    ui::print_blank();
    Ok(())
}
