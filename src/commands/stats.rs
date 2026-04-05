//! Usage-statistics report — `g stats`.
//!
//! Aggregates data from two sources:
//!
//! 1. **SQLite database** (`~/.config/g/g.db`) — command runs, commits recorded
//!    via `g commit`, branch events, workspace events, stack events.
//! 2. **`git log`** — real commit dates (all branches) for the heatmap, and
//!    per-commit line-change counts for the sparkline chart.
//!
//! Sections rendered:
//! - Overview totals + streak information
//! - GitHub-style commit heatmap (last 52 weeks)
//! - Lines added / removed sparkline (last 30 commits, current branch)
//! - Top commands by frequency (horizontal bar chart)
//! - Conventional-commit type distribution (bar chart)
//! - Repository activity ranking (bar chart)
//! - Activity-by-hour heatmap (24-hour chart)

use std::collections::HashMap;

use anyhow::Result;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use rusqlite::Connection;

use crate::cli::StatsArgs;
use crate::commands::git as git_cmd;
use crate::storage::stats as db;
use crate::ui;
use crate::ui::{terminal_width, INDENT};

// ─── Sparkline character set ──────────────────────────────────────────────────

/// Eight increasing block heights used for sparkline charts.
const SPARK: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

// ─── Entry point ─────────────────────────────────────────────────────────────

/// Render the full usage-statistics report.
///
/// # Errors
///
/// Returns an error if the database cannot be queried.
pub fn stats(conn: &Connection, args: &StatsArgs) -> Result<()> {
    ui::print_blank();

    section_overview(conn)?;

    if !args.no_git {
        section_heatmap();
        section_lines_chart();
    }

    section_command_frequency(conn)?;
    section_commit_types(conn)?;
    section_repo_activity(conn)?;
    section_active_hours(conn)?;

    ui::print_blank();
    Ok(())
}

// ─── Section: Overview ───────────────────────────────────────────────────────

fn section_overview(conn: &Connection) -> Result<()> {
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

// ─── Section: Commit heatmap ─────────────────────────────────────────────────

fn section_heatmap() {
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
    // Layout: INDENT (2) + "Mo " (3) + num_weeks * 2 chars.
    let max_weeks = ((terminal_width().saturating_sub(5)) / 2).min(52).max(8);
    let num_weeks = max_weeks;

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
    println!("{}{}", INDENT, ui::muted(&header_str));

    // ── Day rows ──────────────────────────────────────────────────────────────

    let day_labels = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

    for dow in 0..7usize {
        let label = day_labels[dow];
        let mut row = format!("{}{} ", INDENT, ui::muted(label));

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
        INDENT,
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

// ─── Section: Lines added / removed sparkline ─────────────────────────────────

fn section_lines_chart() {
    let stats = fetch_git_line_stats(60);
    if stats.is_empty() {
        return;
    }

    ui::print_fieldset("Lines Changed — Last 60 Commits");
    ui::print_blank();

    let max_added = stats.iter().map(|(a, _)| *a).max().unwrap_or(1).max(1);
    let max_removed = stats.iter().map(|(_, r)| *r).max().unwrap_or(1).max(1);

    // Added sparkline
    let added_spark: String = stats
        .iter()
        .map(|(a, _)| value_to_spark(*a, max_added))
        .collect();

    // Removed sparkline
    let removed_spark: String = stats
        .iter()
        .map(|(_, r)| value_to_spark(*r, max_removed))
        .collect();

    // Peak values for the annotation
    let total_added: u64 = stats.iter().map(|(a, _)| a).sum();
    let total_removed: u64 = stats.iter().map(|(_, r)| r).sum();

    println!(
        "{}{}  {}   peak {}  total {}",
        INDENT,
        ui::success_bold("+ Added  "),
        format!("\x1b[32m{added_spark}\x1b[0m"),
        ui::success(&fmt_n(max_added as i64)),
        ui::success(&fmt_n(total_added as i64)),
    );
    println!(
        "{}{}  {}   peak {}  total {}",
        INDENT,
        ui::danger("- Removed"),
        format!("\x1b[31m{removed_spark}\x1b[0m"),
        ui::danger(&fmt_n(max_removed as i64)),
        ui::danger(&fmt_n(total_removed as i64)),
    );
    println!(
        "{}{}",
        INDENT,
        ui::muted("  oldest ◄──────────────────────────────────────────────── newest"),
    );

    ui::print_blank();
}

/// Map a value onto the 8-level sparkline character set.
fn value_to_spark(val: u64, max: u64) -> char {
    if max == 0 || val == 0 {
        return SPARK[0];
    }
    let idx = ((val as f64 / max as f64) * (SPARK.len() - 1) as f64).round() as usize;
    SPARK[idx.min(SPARK.len() - 1)]
}

/// Run `git log --format=COMMIT --shortstat -n N HEAD` and return per-commit
/// `(lines_added, lines_removed)` in **chronological** order (oldest first).
fn fetch_git_line_stats(n: usize) -> Vec<(u64, u64)> {
    let n_str = format!("-n{n}");
    let output = git_cmd::git_output(&["log", "--format=COMMIT", "--shortstat", &n_str, "HEAD"])
        .unwrap_or_default();

    let mut result: Vec<(u64, u64)> = Vec::new();
    let mut cur_added = 0u64;
    let mut cur_removed = 0u64;
    let mut in_commit = false;

    for line in output.lines() {
        if line == "COMMIT" {
            if in_commit {
                result.push((cur_added, cur_removed));
            }
            cur_added = 0;
            cur_removed = 0;
            in_commit = true;
        } else if line.contains("insertion") || line.contains("deletion") {
            if let Some(a) = parse_insertions(line) {
                cur_added = a;
            }
            if let Some(d) = parse_deletions(line) {
                cur_removed = d;
            }
        }
    }
    if in_commit {
        result.push((cur_added, cur_removed));
    }

    result.reverse(); // git log is newest-first; flip to chronological
    result
}

/// Extract insertion count from a git `--shortstat` line.
fn parse_insertions(line: &str) -> Option<u64> {
    let i = line.find("insertion")?;
    line[..i].split_whitespace().last()?.parse().ok()
}

/// Extract deletion count from a git `--shortstat` line.
fn parse_deletions(line: &str) -> Option<u64> {
    let i = line.find("deletion")?;
    line[..i].split_whitespace().last()?.parse().ok()
}

// ─── Section: Top commands ────────────────────────────────────────────────────

fn section_command_frequency(conn: &Connection) -> Result<()> {
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

// ─── Section: Commit types ────────────────────────────────────────────────────

fn section_commit_types(conn: &Connection) -> Result<()> {
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

/// Like `render_bar_chart` but colours each bar based on conventional-commit type.
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
            INDENT,
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

// ─── Section: Repository activity ────────────────────────────────────────────

fn section_repo_activity(conn: &Connection) -> Result<()> {
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

// ─── Section: Activity by hour ────────────────────────────────────────────────

fn section_active_hours(conn: &Connection) -> Result<()> {
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

    // Top axis
    print!("{}", INDENT);
    for h in 0..24 {
        let count = by_hour[h];
        let height = ((count as f64 / max_count as f64) * BAR_HEIGHT as f64).round() as usize;
        // Print from top of the chart
        print!(
            "{}",
            if height == BAR_HEIGHT {
                format!(" {}", ui::primary_bold("▉"))
            } else {
                format!("  ")
            }
        );
    }
    println!();

    for row in (0..BAR_HEIGHT).rev() {
        print!("{}", INDENT);
        for h in 0..24 {
            let count = by_hour[h];
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
    print!("{}", INDENT);
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

// ─── Generic bar chart ────────────────────────────────────────────────────────

/// Render a horizontal bar chart to stdout.
///
/// Each bar uses `█` for filled cells and `░` for empty cells.
/// The bar width is scaled so the largest value fills `bar_width` columns.
fn render_bar_chart(items: &[(String, i64)], bar_width: usize) {
    if items.is_empty() {
        ui::print_info("No data recorded yet.");
        return;
    }

    let max_count = items.iter().map(|(_, n)| *n).max().unwrap_or(1).max(1);
    let max_label = items
        .iter()
        .map(|(l, _)| console::measure_text_width(l))
        .max()
        .unwrap_or(0);

    for (label, count) in items {
        let filled = (*count as usize * bar_width) / max_count as usize;
        let bar = format!(
            "{}{}",
            ui::success(&"█".repeat(filled)),
            ui::muted(&"░".repeat(bar_width - filled))
        );
        let label_pad = " ".repeat(max_label - console::measure_text_width(label));
        println!(
            "{}{}{}  {}  {}",
            INDENT,
            ui::paint_text(label),
            label_pad,
            bar,
            ui::muted(&count.to_string()),
        );
    }
}

// ─── Numeric formatting ───────────────────────────────────────────────────────

/// Format a duration given in milliseconds into a human-readable string.
///
/// - < 1 000 ms   → `"N ms"`
/// - < 60 000 ms  → `"N.N s"`
/// - otherwise    → `"N min N s"`
fn fmt_duration_ms(ms: f64) -> String {
    if ms < 1_000.0 {
        format!("{:.0} ms", ms)
    } else if ms < 60_000.0 {
        format!("{:.1} s", ms / 1_000.0)
    } else {
        let total_s = (ms / 1_000.0) as u64;
        format!("{} min {} s", total_s / 60, total_s % 60)
    }
}

/// Format a large integer with thousands separators: `1234567` → `"1,234,567"`.
fn fmt_n(n: i64) -> String {
    if n < 0 {
        return format!("-{}", fmt_n(-n));
    }
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}
