//! Compare two branches with commit stats, file stats, and optional diffs.
//!
//! ## Tutorial overview
//!
//! This module handles the `g compare` command.  It:
//!
//! 1. Determines the base and head branches (from CLI args or sensible defaults).
//! 2. Optionally fetches from remotes so the counts reflect the current remote state.
//! 3. Uses `git rev-list --count` to compute how many commits each branch is ahead
//!    or behind the other.
//! 4. Displays the differences as a list of commits, a file-level diffstat, or a
//!    full patch, depending on the flags the user passes.
//!
//! ## Rust concepts used here
//!
//! - `unwrap_or_else` for providing defaults when an `Option` is `None`.
//! - String formatting with `format!` for complex CLI output.
//! - Iterators (`map`, `collect`, `join`) to process multi-line git output.
//! - Module delegation: [`show_full_diff`] calls `enhanced_diff` from the `git`
//!   module rather than duplicating the diff-tool selection logic.

use anyhow::Result;
use colored::Colorize;

use crate::cli::CompareArgs;
use crate::commands::git as gitcmd;
use crate::config;
use crate::ui;

/// Entry point for `g compare`.
///
/// Compares `base` against `head` (defaulting to the configured default branch
/// and the current branch, respectively) and prints the result to stdout.
///
/// # Errors
///
/// Returns an error if:
/// - The config cannot be loaded.
/// - An optional `git fetch` fails.
/// - Any git command used for counting or displaying commits fails.
pub fn compare(args: &CompareArgs) -> Result<()> {
    let cfg = config::load()?;

    let current = gitcmd::current_branch().unwrap_or_else(|_| "HEAD".into());

    let base = args.base.clone().unwrap_or_else(gitcmd::default_branch);

    let head = args.head.clone().unwrap_or_else(|| current.clone());

    ui::print_blank();
    println!(
        "  {} {} {} {}",
        "Comparing".bright_black(),
        base.cyan().bold(),
        "→".bright_black(),
        head.green().bold()
    );

    if cfg.general.auto_fetch {
        if !gitcmd::is_dry_run() {
            let pb = ui::spinner("Fetching remotes…");
            let _ = gitcmd::git_output(&["fetch", "--all", "--quiet"]);
            ui::spinner_success(pb, "Fetched");
        } else {
            let _ = gitcmd::git_mutate(
                &["fetch", "--all", "--quiet"],
                "Fetch latest refs from all remotes before comparing",
            );
        }
    }

    // Count how many commits each branch is ahead of the other.
    let ahead_output =
        gitcmd::git_output_lossy(&["rev-list", "--count", &format!("{}..{}", base, head)]);
    let behind_output =
        gitcmd::git_output_lossy(&["rev-list", "--count", &format!("{}..{}", head, base)]);

    let ahead: usize = ahead_output.trim().parse().unwrap_or(0);
    let behind: usize = behind_output.trim().parse().unwrap_or(0);

    ui::print_blank();
    println!("  {}", ui::format_ahead_behind(ahead, behind));

    // ─── Commits ──────────────────────────────────────────────────────────────

    if ahead > 0 && (args.commits || (!args.stat && !args.diff)) {
        show_commits(&base, &head, ahead)?;
    }

    // ─── File stat ────────────────────────────────────────────────────────────

    if args.stat || (!args.diff && !args.commits) {
        show_file_stat(&base, &head)?;
    }

    // ─── Full diff ────────────────────────────────────────────────────────────

    if args.diff {
        show_full_diff(&base, &head)?;
    }

    ui::print_blank();
    Ok(())
}

/// Print a list of commits that are in `head` but not in `base`.
///
/// # Errors
///
/// Returns an error if the underlying `git log` call fails.
fn show_commits(base: &str, head: &str, count: usize) -> Result<()> {
    ui::print_section(
        &format!(
            "Commits ahead ({}) {} {} {}",
            count.to_string().green(),
            head.green().bold(),
            "not in".bright_black(),
            base.cyan()
        ),
        None,
    );

    // Use the same \x01/\x02 sentinel-based format as `enhanced_log` to avoid
    // field-parsing errors when commit subjects contain special characters.
    let fmt = "\x02%h\x01%s\x01%an\x01%ar\x01%D\x02";
    let log_range = format!("{}..{}", base, head);
    let raw = gitcmd::git_output_lossy(&[
        "log",
        &format!("--format={}", fmt),
        "--no-walk=unsorted",
        &log_range,
    ]);

    for line in raw.lines() {
        if let (Some(start), Some(end)) = (line.find('\x02'), line.rfind('\x02')) {
            if start != end {
                let record = &line[start + 1..end];
                let fields: Vec<&str> = record.splitn(5, '\x01').collect();
                if fields.len() >= 4 {
                    let entry = ui::CommitEntry {
                        hash: fields[0].to_string(),
                        subject: fields[1].to_string(),
                        author: fields[2].to_string(),
                        date: fields[3].to_string(),
                        refs: fields.get(4).copied().unwrap_or("").to_string(),
                        graph_prefix: "  ".to_string(),
                    };
                    println!("{}", entry.render(55));
                }
            }
        }
    }
    Ok(())
}

/// Show a diffstat summary (changed files, insertion/deletion counts) between branches.
fn show_file_stat(base: &str, head: &str) -> Result<()> {
    // Three-dot `...` diff compares the tips of both branches against their
    // common ancestor, which is the most intuitive "what changed on each side"
    // view.
    let stat_raw = gitcmd::git_output_lossy(&[
        "diff",
        "--stat",
        "--no-color",
        &format!("{}...{}", base, head),
    ]);

    if stat_raw.trim().is_empty() {
        ui::print_blank();
        println!("  {}", "No file changes between branches.".bright_black());
        return Ok(());
    }

    ui::print_section("Changed Files", None);

    let lines: Vec<&str> = stat_raw.lines().collect();
    let last = lines.len().saturating_sub(1);

    for (i, line) in lines.iter().enumerate() {
        if i == last {
            // Summary line: "12 files changed, 345 insertions(+), 67 deletions(-)"
            ui::print_blank();
            println!("  {}", colorize_summary_line(line));
        } else {
            // File line: "  path/to/file.rs | 12 +++---"
            println!("{}", colorize_file_stat_line(line));
        }
    }
    Ok(())
}

/// Delegate to the enhanced diff for a full patch view.
///
/// # Errors
///
/// Propagates any error from [`gitcmd::enhanced_diff`].
fn show_full_diff(base: &str, head: &str) -> Result<()> {
    ui::print_blank();
    let diff_args = vec![format!("{}...{}", base, head)];
    crate::commands::git::enhanced_diff(&diff_args)
}

/// Colorise a single diffstat file line.
///
/// Input: `"  src/main.rs | 12 +++---"`
/// Output: path in white, bar in colour, counts in green/red.
fn colorize_file_stat_line(line: &str) -> String {
    if let Some(pipe_pos) = line.rfind('|') {
        let path_part = &line[..pipe_pos];
        let stat_part = &line[pipe_pos + 1..];

        let (added_count, deleted_count) = parse_stat_counts(stat_part);

        let bar = ui::render_stat_bar(added_count, deleted_count, 20);
        let counts = format!(
            "{} {} {}",
            (added_count + deleted_count).to_string().bright_black(),
            ui::color_added(added_count as i64),
            ui::color_deleted(deleted_count as i64)
        );

        format!(
            "  {}{}  {} {}",
            path_part.white(),
            "|".bright_black(),
            counts,
            bar
        )
    } else {
        format!("  {}", line.bright_black())
    }
}

/// Count `+` and `-` characters in a diffstat stat part to derive insertion/deletion totals.
fn parse_stat_counts(stat: &str) -> (usize, usize) {
    let added = stat.chars().filter(|&c| c == '+').count();
    let deleted = stat.chars().filter(|&c| c == '-').count();
    (added, deleted)
}

/// Colorise the final summary line of a diffstat output.
///
/// Input: `"12 files changed, 345 insertions(+), 67 deletions(-)"`
fn colorize_summary_line(line: &str) -> String {
    line.split(", ")
        .map(|part| {
            if part.contains("insertion") {
                part.green().to_string()
            } else if part.contains("deletion") {
                part.red().to_string()
            } else {
                part.white().to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(&", ".bright_black().to_string())
}
