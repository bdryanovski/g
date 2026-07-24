//! `g push` — enhanced push with styled progress output.
//!
//! Captures git push progress and renders it as a clean tree-style display
//! with a final summary showing commit range and transfer stats.

use anyhow::{Context, Result};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use super::exec::git_exe;
use super::repo::current_branch;
use crate::ui;

/// Final result of the push operation.
#[allow(dead_code)]
struct PushResult {
    /// Whether push succeeded
    success: bool,
    /// Branch that was pushed
    branch: String,
    /// Remote branch (e.g., "origin/main")
    remote_branch: String,
    /// Old commit hash
    old_hash: Option<String>,
    /// New commit hash
    new_hash: Option<String>,
    /// Compression stats
    compress_objects: Option<u32>,
    /// Write stats
    write_objects: Option<u32>,
    /// Size written
    write_size: Option<String>,
    /// Write speed
    write_speed: Option<String>,
    /// Delta resolution stats
    resolve_deltas: Option<u32>,
    /// Error message if failed
    error: Option<String>,
    /// Whether repo is up to date (nothing to push)
    up_to_date: bool,
}

/// Run `g push` with enhanced output.
///
/// Passthrough for complex flags; enhanced display for simple pushes.
pub fn enhanced_push(extra_args: &[String]) -> Result<()> {
    // For complex flags, just passthrough to git
    let passthrough_flags = [
        "--force",
        "-f",
        "--force-with-lease",
        "--delete",
        "-d",
        "--tags",
        "--all",
        "--mirror",
        "--dry-run",
        "-n",
        "--set-upstream",
        "-u",
    ];

    let needs_passthrough = extra_args
        .iter()
        .any(|a| passthrough_flags.iter().any(|f| a.starts_with(f)));

    if needs_passthrough {
        return super::exec::passthrough(
            &std::iter::once("push".to_string())
                .chain(extra_args.iter().cloned())
                .collect::<Vec<_>>(),
        );
    }

    let branch = current_branch().unwrap_or_else(|_| "HEAD".to_string());

    // Get upstream info before push
    let upstream = super::exec::git_output_lossy(&["rev-parse", "--abbrev-ref", "@{upstream}"]);
    let remote_branch = if upstream.is_empty() || upstream.contains("fatal") {
        format!("origin/{}", branch)
    } else {
        upstream
    };

    ui::print_blank();
    println!(
        "  {} {} {} {}",
        ui::muted("◯"),
        ui::paint_text(&branch),
        ui::muted("→"),
        ui::color_branch(&remote_branch)
    );
    ui::print_blank();

    // Build args
    let mut args = vec!["push", "--progress"];
    let extra_refs: Vec<&str> = extra_args.iter().map(|s| s.as_str()).collect();
    args.extend(extra_refs);

    // Spawn git push with piped stderr (progress goes to stderr)
    let mut child = Command::new(git_exe())
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn git push")?;

    let stderr = child.stderr.take().context("failed to capture stderr")?;

    // Track progress phases
    let mut compress_done = false;
    let mut write_done = false;
    let mut resolve_done = false;
    let mut result = PushResult {
        success: false,
        branch: branch.clone(),
        remote_branch: remote_branch.clone(),
        old_hash: None,
        new_hash: None,
        compress_objects: None,
        write_objects: None,
        write_size: None,
        write_speed: None,
        resolve_deltas: None,
        error: None,
        up_to_date: false,
    };

    // Read stderr line by line (git uses \r for progress updates)
    let reader = BufReader::new(stderr);
    for line_result in reader.split(b'\r') {
        let bytes = match line_result {
            Ok(b) => b,
            Err(_) => continue,
        };
        let line = String::from_utf8_lossy(&bytes);

        for part in line.split('\n') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            // Check for "Everything up-to-date"
            if part.contains("Everything up-to-date") {
                result.up_to_date = true;
                result.success = true;
                continue;
            }

            // Parse compression line: "Compressing objects: 100% (6/6), done."
            if part.contains("Compressing objects:") && part.contains("done") && !compress_done {
                compress_done = true;
                if let Some(counts) = parse_object_counts(part) {
                    result.compress_objects = Some(counts);
                }
                print_progress_line("Compressing", &format!("{} objects", counts_display(result.compress_objects)), true);
            }

            // Parse writing line: "Writing objects: 100% (6/6), 6.21 KiB | 3.11 MiB/s, done."
            if part.contains("Writing objects:") && part.contains("done") && !write_done {
                write_done = true;
                if let Some(counts) = parse_object_counts(part) {
                    result.write_objects = Some(counts);
                }
                // Parse size and speed
                if let Some((size, speed)) = parse_transfer_stats(part) {
                    result.write_size = Some(size);
                    result.write_speed = Some(speed);
                }
                let stats = format!(
                    "{} @ {}",
                    result.write_size.as_deref().unwrap_or("?"),
                    result.write_speed.as_deref().unwrap_or("?")
                );
                print_progress_line("Writing", &stats, true);
            }

            // Parse resolving line: "remote: Resolving deltas: 100% (4/4), completed with 4 local objects."
            if part.contains("Resolving deltas:") && (part.contains("done") || part.contains("completed")) && !resolve_done {
                resolve_done = true;
                if let Some(counts) = parse_object_counts(part) {
                    result.resolve_deltas = Some(counts);
                }
                print_progress_line("Resolving", &format!("{} deltas", counts_display(result.resolve_deltas)), true);
            }

            // Parse ref update line: "   eb36938..412a832  main -> main"
            if part.contains("..") && part.contains("->") {
                if let Some((old, new)) = parse_ref_update(part) {
                    result.old_hash = Some(old);
                    result.new_hash = Some(new);
                }
            }

            // Check for errors
            if part.contains("error:") || part.contains("fatal:") || part.contains("rejected") {
                result.error = Some(part.to_string());
            }
        }
    }

    // Wait for process to finish
    let status = child.wait().context("failed to wait for git push")?;
    result.success = status.success();

    ui::print_blank();

    // Print final summary
    if result.up_to_date {
        println!("  {} Already up to date", ui::success_bold("✓"));
    } else if result.success {
        let hash_range = match (&result.old_hash, &result.new_hash) {
            (Some(old), Some(new)) => format!("{}..{}", ui::color_hash(old), ui::color_hash(new)),
            _ => String::new(),
        };

        // Count commits pushed
        let commit_count = if let (Some(old), Some(new)) = (&result.old_hash, &result.new_hash) {
            let count_output = super::exec::git_output_lossy(&[
                "rev-list",
                "--count",
                &format!("{}..{}", old, new),
            ]);
            count_output.trim().parse::<u32>().unwrap_or(0)
        } else {
            0
        };

        let commit_word = if commit_count == 1 { "commit" } else { "commits" };

        println!(
            "  {} Pushed {} ({} {})",
            ui::success_bold("✓"),
            hash_range,
            commit_count,
            commit_word
        );
    } else {
        println!("  {} Push failed", ui::danger_bold("✗"));
        if let Some(err) = &result.error {
            ui::print_blank();
            println!("  {}", ui::danger(err));
            
            // Provide hints for common errors
            if err.contains("rejected") || err.contains("non-fast-forward") {
                ui::print_blank();
                println!(
                    "  {} Run {} first, then try again.",
                    ui::muted("Hint:"),
                    ui::accent("`g pull`")
                );
            }
        }
    }

    ui::print_blank();

    if result.success {
        Ok(())
    } else {
        anyhow::bail!("push failed")
    }
}

/// Print a progress line with tree-style prefix.
fn print_progress_line(phase: &str, detail: &str, done: bool) {
    let prefix = if done { "├─" } else { "│ " };
    let status = if done {
        ui::success_bold("✓")
    } else {
        ui::muted("…")
    };
    println!(
        "  {} {} {} {}",
        ui::muted(prefix),
        status,
        ui::paint_text(phase),
        ui::muted(detail)
    );
}

/// Parse object counts from a progress line like "Compressing objects: 100% (6/6), done."
fn parse_object_counts(line: &str) -> Option<u32> {
    // Look for pattern like "(6/6)" and extract the second number
    let start = line.find('(')?;
    let end = line.find(')')?;
    let counts = &line[start + 1..end];
    let parts: Vec<&str> = counts.split('/').collect();
    if parts.len() == 2 {
        parts[1].trim().parse().ok()
    } else {
        None
    }
}

/// Parse transfer stats from writing line.
fn parse_transfer_stats(line: &str) -> Option<(String, String)> {
    // Pattern: "6.21 KiB | 3.11 MiB/s"
    // Find the part after the counts: ", 6.21 KiB | 3.11 MiB/s, done."
    let parts: Vec<&str> = line.split(',').collect();
    for part in parts {
        let part = part.trim();
        if part.contains('|') && (part.contains("KiB") || part.contains("MiB") || part.contains("B")) {
            let sub_parts: Vec<&str> = part.split('|').collect();
            if sub_parts.len() == 2 {
                return Some((sub_parts[0].trim().to_string(), sub_parts[1].trim().to_string()));
            }
        }
    }
    None
}

/// Parse ref update line like "   eb36938..412a832  main -> main"
fn parse_ref_update(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    // Find the hash range
    let parts: Vec<&str> = line.split_whitespace().collect();
    for part in parts {
        if part.contains("..") {
            let hashes: Vec<&str> = part.split("..").collect();
            if hashes.len() == 2 {
                return Some((hashes[0].to_string(), hashes[1].to_string()));
            }
        }
    }
    None
}

/// Format object count for display.
fn counts_display(count: Option<u32>) -> String {
    count.map(|c| c.to_string()).unwrap_or_else(|| "?".to_string())
}
// test
