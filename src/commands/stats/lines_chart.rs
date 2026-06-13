//! Section: **Lines Changed** — `+`/`-` sparkline over the last 60 commits.

use crate::commands::git as git_cmd;
use crate::ui;
use crate::ui::indent;

use super::shared::{fmt_n, SPARK};

pub(super) fn run() {
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
        "{}{}  \x1b[32m{added_spark}\x1b[0m   peak {}  total {}",
        indent(),
        ui::success_bold("+ Added  "),
        ui::success(&fmt_n(max_added as i64)),
        ui::success(&fmt_n(total_added as i64)),
    );
    println!(
        "{}{}  \x1b[31m{removed_spark}\x1b[0m   peak {}  total {}",
        indent(),
        ui::danger("- Removed"),
        ui::danger(&fmt_n(max_removed as i64)),
        ui::danger(&fmt_n(total_removed as i64)),
    );
    println!(
        "{}{}",
        indent(),
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
