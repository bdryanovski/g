//! Section: **Activity by Hour** — 24-column vertical bar chart of when (UTC)
//! during the day you tend to run commands.

use anyhow::Result;

use crate::commands::Ctx;
use crate::storage::stats as db;
use crate::ui;
use crate::ui::indent;

use super::shared::fmt_n;

pub(super) fn run(ctx: &Ctx) -> Result<()> {
    let conn = ctx.conn;
    let raw = db::activity_by_hour(conn)?;
    if raw.is_empty() {
        return Ok(());
    }

    ui::print_fieldset("Activity by Hour  (UTC)");
    ui::print_blank();

    // Build a full 0-23 array.
    let mut by_hour = [0i64; 24];
    for (h, cnt) in &raw {
        if (*h as usize) < 24 {
            by_hour[*h as usize] = *cnt;
        }
    }

    let max_count = *by_hour.iter().max().unwrap_or(&1).max(&1);
    const BAR_HEIGHT: usize = 6; // rows in the vertical chart

    // Render as a vertical bar chart: print from top down, each row is a
    // threshold level.
    let hour_labels: Vec<String> = (0..24).map(|h| format!("{:02}", h)).collect();

    // Top axis: mark every column whose bar reaches the chart ceiling.
    print!("{}", indent());
    for &count in &by_hour {
        let height = ((count as f64 / max_count as f64) * BAR_HEIGHT as f64).round() as usize;
        if height == BAR_HEIGHT {
            print!(" {}", ui::primary_bold("▉"));
        } else {
            print!("  ");
        }
    }
    println!();

    for row in (0..BAR_HEIGHT).rev() {
        print!("{}", indent());
        for (h, &count) in by_hour.iter().enumerate() {
            let height = ((count as f64 / max_count as f64) * BAR_HEIGHT as f64).round() as usize;
            if height > row {
                let ch = if h % 2 == 0 {
                    ui::primary("▉")
                } else {
                    ui::success("▉")
                };
                print!(" {ch}");
            } else {
                print!("  ");
            }
        }
        println!();
    }

    // Hour labels
    print!("{}", indent());
    for label in &hour_labels {
        print!(" {}", ui::muted(label));
    }
    println!();

    // Peak annotation
    let peak_hour = by_hour
        .iter()
        .enumerate()
        .max_by_key(|(_, &v)| v)
        .map(|(h, _)| h)
        .unwrap_or(0);
    ui::print_blank();
    ui::print_tip(&format!(
        "Peak hour: {:02}:00 UTC  ({} commands)",
        peak_hour,
        fmt_n(by_hour[peak_hour])
    ));

    ui::print_blank();
    Ok(())
}
