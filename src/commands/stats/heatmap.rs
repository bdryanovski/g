//! Section: **Commit Heatmap** — GitHub-style 52-week contribution grid.

use std::collections::HashMap;

use chrono::{Datelike, Duration, NaiveDate, Utc};

use crate::commands::git as git_cmd;
use crate::ui;
use crate::ui::{indent, terminal_width};

pub(super) fn run() {
    let data = fetch_commit_dates();
    // Always render the section header; skip only the grid when there is no git repo.

    ui::print_fieldset("Commit Heatmap — Last 52 Weeks");
    ui::print_blank();

    if data.is_empty() {
        ui::print_info("No git history found in this repository.");
        ui::print_blank();
        return;
    }

    let today = Utc::now().date_naive();

    // Align the grid start to the Monday that contains (today - 364 days).
    let raw_start = today - Duration::days(364);
    let offset = raw_start.weekday().num_days_from_monday() as i64;
    let start = raw_start - Duration::days(offset);

    // Clamp the number of weeks to what the terminal can fit.
    // Layout: indent() (2) + "Mo " (3) + num_weeks * 2 chars.
    let num_weeks = (terminal_width().saturating_sub(5) / 2).clamp(8, 52);

    // ── Month header ──────────────────────────────────────────────────────────

    // Pre-compute (week_index, month) pairs at which the month changes.
    let prefix_len = 3usize; // "Mo " — same width as day-label below
    let header_len = prefix_len + num_weeks * 2;

    let mut month_starts: Vec<(usize, u32)> = Vec::new();
    let mut prev_month = 0u32;
    for w in 0..num_weeks {
        let m = (start + Duration::days((w * 7) as i64)).month();
        if m != prev_month {
            month_starts.push((w, m));
            prev_month = m;
        }
    }

    // Write month names into a char vec, skipping those with insufficient room.
    let mut header_chars: Vec<char> = vec![' '; header_len];
    for (i, &(w, m)) in month_starts.iter().enumerate() {
        let name = month_abbr(m);
        let pos = prefix_len + w * 2;
        // Space available: distance to the next month label (or end of grid).
        let next_pos = month_starts
            .get(i + 1)
            .map(|&(nw, _)| prefix_len + nw * 2)
            .unwrap_or(header_len);
        let available = next_pos.saturating_sub(pos);
        if available < name.len() {
            continue; // not enough room — skip this label to avoid overlap
        }
        for (j, ch) in name.chars().enumerate() {
            if pos + j < header_len {
                header_chars[pos + j] = ch;
            }
        }
    }
    let header_str: String = header_chars.iter().collect();
    println!("{}{}", indent(), ui::muted(&header_str));

    // ── Day rows ──────────────────────────────────────────────────────────────

    let day_labels = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

    for (dow, label) in day_labels.iter().enumerate() {
        let mut row = format!("{}{} ", indent(), ui::muted(label));

        for w in 0..num_weeks {
            let date = start + Duration::days((w * 7 + dow) as i64);
            if date > today {
                // Future cell — blank
                row.push_str("  ");
            } else {
                let count = data.get(&date).copied().unwrap_or(0);
                row.push_str(&heatmap_cell(count));
            }
        }

        println!("{}", row);
    }

    // ── Legend ────────────────────────────────────────────────────────────────
    ui::print_blank();
    println!(
        "{}{}  {}  {}  {}  {}  {}  {}  {}  {}",
        indent(),
        ui::muted("Legend:"),
        heatmap_cell(0).trim_end(),
        ui::muted("0"),
        heatmap_cell(1).trim_end(),
        ui::muted("1–2"),
        heatmap_cell(4).trim_end(),
        ui::muted("3–5"),
        heatmap_cell(8).trim_end(),
        ui::muted("6+"),
    );
    ui::print_blank();
}

/// Return a colored terminal cell (block + trailing space) for the given commit count.
fn heatmap_cell(count: u32) -> String {
    let ch = match count {
        0 => '░',
        1..=2 => '▒',
        3..=5 => '▓',
        _ => '█',
    };
    let colored = match count {
        0 => format!("\x1b[38;5;238m{ch}\x1b[0m"), // very dark gray
        1..=2 => format!("\x1b[38;5;28m{ch}\x1b[0m"), // dark green
        3..=5 => format!("\x1b[38;5;34m{ch}\x1b[0m"), // medium green
        _ => format!("\x1b[38;5;46m{ch}\x1b[0m"),  // bright green
    };
    format!("{colored} ")
}

/// Short (3-char) month abbreviation.
fn month_abbr(m: u32) -> &'static str {
    match m {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "   ",
    }
}

/// Run `git log --all --format=%ad --date=short --since=...` and return a
/// per-day commit count map for the last 365 days.
fn fetch_commit_dates() -> HashMap<NaiveDate, u32> {
    let mut map: HashMap<NaiveDate, u32> = HashMap::new();
    let since = (Utc::now() - Duration::days(365))
        .format("%Y-%m-%d")
        .to_string();
    let output = git_cmd::git_output(&[
        "log",
        "--all",
        "--format=%ad",
        "--date=short",
        &format!("--since={since}"),
    ])
    .unwrap_or_default();

    for line in output.lines() {
        if let Ok(date) = NaiveDate::parse_from_str(line.trim(), "%Y-%m-%d") {
            *map.entry(date).or_insert(0) += 1;
        }
    }
    map
}
