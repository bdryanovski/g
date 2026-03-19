//! Compare two branches with commit stats, file stats, and optional diffs.

use anyhow::Result;
use colored::Colorize;

use crate::cli::CompareArgs;
use crate::commands::git as gitcmd;
use crate::config;
use crate::ui;

/// Entry point for `g compare`.
pub fn compare(args: &CompareArgs) -> Result<()> {
    let cfg = config::load()?;

    let current = gitcmd::current_branch().unwrap_or_else(|_| "HEAD".into());

    let base = args
        .base
        .clone()
        .unwrap_or_else(|| gitcmd::default_branch());

    let head = args.head.clone().unwrap_or_else(|| current.clone());

    println!();
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
            pb.finish_and_clear();
        } else {
            let _ = gitcmd::git_mutate(
                &["fetch", "--all", "--quiet"],
                "Fetch latest refs from all remotes before comparing",
            );
        }
    }

    // Count commits ahead/behind.
    let ahead_output =
        gitcmd::git_output_lossy(&["rev-list", "--count", &format!("{}..{}", base, head)]);
    let behind_output =
        gitcmd::git_output_lossy(&["rev-list", "--count", &format!("{}..{}", head, base)]);

    let ahead: usize = ahead_output.trim().parse().unwrap_or(0);
    let behind: usize = behind_output.trim().parse().unwrap_or(0);

    println!();
    println!("  {}", ui::format_ahead_behind(ahead, behind));

    // ─── Commits ──────────────────────────────────────────────────────────────

    if ahead > 0 && (args.commits || !args.stat && !args.diff) {
        show_commits(&base, &head, ahead)?;
    }

    // ─── File Stat ────────────────────────────────────────────────────────────

    if args.stat || (!args.diff && !args.commits) {
        show_file_stat(&base, &head)?;
    }

    // ─── Full Diff ────────────────────────────────────────────────────────────

    if args.diff {
        show_full_diff(&base, &head)?;
    }

    println!();
    Ok(())
}

/// Print a list of commits that are in `head` but not in `base`.
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

/// Show a diffstat summary between branches.
fn show_file_stat(base: &str, head: &str) -> Result<()> {
    // Use three-dot diff for comparing tips of both branches
    let stat_raw = gitcmd::git_output_lossy(&[
        "diff",
        "--stat",
        "--no-color",
        &format!("{}...{}", base, head),
    ]);

    if stat_raw.trim().is_empty() {
        println!();
        println!("  {}", "No file changes between branches.".bright_black());
        return Ok(());
    }

    ui::print_section("Changed Files", None);

    let lines: Vec<&str> = stat_raw.lines().collect();
    let last = lines.len().saturating_sub(1);

    for (i, line) in lines.iter().enumerate() {
        if i == last {
            // Summary line: "12 files changed, 345 insertions(+), 67 deletions(-)"
            println!();
            println!("  {}", colorize_summary_line(line));
        } else {
            // File line: "  path/to/file.rs | 12 +++---"
            println!("{}", colorize_file_stat_line(line));
        }
    }
    Ok(())
}

/// Delegate to the enhanced diff for a full patch view.
fn show_full_diff(base: &str, head: &str) -> Result<()> {
    println!();
    // Run enhanced diff
    let diff_args = vec![format!("{}...{}", base, head)];
    crate::commands::git::enhanced_diff(&diff_args)
}

/// Colorize a single diffstat line.
fn colorize_file_stat_line(line: &str) -> String {
    if let Some(pipe_pos) = line.rfind('|') {
        let path_part = &line[..pipe_pos];
        let stat_part = &line[pipe_pos + 1..];

        // Parse added/deleted from the bar
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

fn parse_stat_counts(stat: &str) -> (usize, usize) {
    let added = stat.chars().filter(|&c| c == '+').count();
    let deleted = stat.chars().filter(|&c| c == '-').count();
    (added, deleted)
}

/// Colorize the final summary line of a diffstat output.
fn colorize_summary_line(line: &str) -> String {
    // "12 files changed, 345 insertions(+), 67 deletions(-)"
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

// TODO(compare): Add `--base`/`--head` validation (ensure branches exist before running).
// TODO(compare): Support `--name-only` or `--name-status` quick modes.
