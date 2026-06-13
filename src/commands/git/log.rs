//! `g log` — colourised, opinionated replacement for `git log`.
//!
//! Parses `git log --pretty=format:` output with ASCII control-character
//! field separators so subject lines containing special characters are still
//! split reliably, then renders each commit via [`ui::CommitEntry`] with the
//! graph art (if enabled) colourised.

use anyhow::Result;

use crate::config;
use crate::ui;

use super::exec::git_output_lossy;

/// Parse and pretty-print `git log` with colours, graph art, and aligned columns.
pub fn enhanced_log(extra_args: &[String]) -> Result<()> {
    let cfg = config::load().unwrap_or_default();

    // Special ASCII control characters chosen to be collision-free with typical
    // commit message content.
    const SEP: &str = "\x01"; // Start of Heading — field separator
    const REC: &str = "\x02"; // Start of Text — record separator

    // Format: REC + full_hash + SEP + short_hash + SEP + subject + SEP +
    //         author_name + SEP + rel_date + SEP + ref_names + REC
    let fmt = format!(
        "{}%H{}%h{}%s{}%an{}%ar{}%D{}",
        REC, SEP, SEP, SEP, SEP, SEP, REC
    );

    let mut args = vec!["log".to_string(), format!("--pretty=format:{}", fmt)];

    // Add --graph unless the user explicitly requested --no-graph.
    let has_graph = cfg.ui.show_graph && !extra_args.contains(&"--no-graph".to_string());
    if has_graph
        && !extra_args
            .iter()
            .any(|a| a == "--graph" || a == "--no-graph")
    {
        args.push("--graph".to_string());
    }

    // Apply a default commit limit unless the user passed -n/--max-count/--all.
    let has_limit = extra_args
        .iter()
        .any(|a| a.starts_with("-n") || a.starts_with("--max-count") || a.starts_with("--all"));
    if !has_limit {
        args.push(format!("-n{}", cfg.ui.log_limit));
    }

    args.extend_from_slice(extra_args);

    let output = git_output_lossy(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>());

    if output.is_empty() {
        ui::print_indented(&ui::muted("No commits found."));
        return Ok(());
    }

    ui::print_blank(); // top padding

    // Calculate the subject column width once for the whole log run so all
    // entries align regardless of individual graph-prefix lengths.
    let subject_width = ui::commit_subject_width(has_graph);

    for line in output.lines() {
        // Lines that contain a commit record are bounded by two \x02 bytes.
        if let (Some(start), Some(end)) = (line.find('\x02'), line.rfind('\x02')) {
            if start != end {
                let record = &line[start + 1..end];
                let graph_prefix = &line[..start];
                let fields: Vec<&str> = record.splitn(7, '\x01').collect();

                if fields.len() >= 6 {
                    let short_hash = fields[1];
                    let subject = fields[2];
                    let author = fields[3];
                    let rel_date = fields[4];
                    let refs = fields[5];

                    let entry = ui::CommitEntry {
                        hash: short_hash.to_string(),
                        subject: subject.to_string(),
                        author: author.to_string(),
                        date: rel_date.to_string(),
                        refs: refs.to_string(),
                        graph_prefix: graph_prefix.to_string(),
                    };

                    ui::print_line(&entry.render(subject_width));
                    continue;
                }
            }
        }

        // Graph-only lines (no commit data) — colourised and printed as-is.
        if !line.trim().is_empty() {
            ui::print_line(&ui::colorize_graph(line));
        }
    }

    ui::print_blank(); // bottom padding
    Ok(())
}
