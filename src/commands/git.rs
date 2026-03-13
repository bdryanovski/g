use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::process::{Command, Stdio};

use crate::config;
use crate::ui;

// ─── Git Executable ───────────────────────────────────────────────────────────

pub fn git_exe() -> String {
    let cfg = config::load().unwrap_or_default();
    cfg.general
        .git_path
        .unwrap_or_else(|| "git".to_string())
}

/// Run git and return stdout as a String
pub fn git_output(args: &[&str]) -> Result<String> {
    let out = Command::new(git_exe())
        .args(args)
        .output()
        .with_context(|| format!("Failed to run git {:?}", args))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        bail!("{}", stderr)
    }
}

/// Run git and return output even on non-zero exit
pub fn git_output_lossy(args: &[&str]) -> String {
    Command::new(git_exe())
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Run git, streaming stdout/stderr directly to the terminal (for passthrough)
pub fn passthrough(args: &[String]) -> Result<()> {
    let cfg = config::load().unwrap_or_default();

    // Check aliases first
    if let Some(first) = args.first() {
        if let Some(alias_target) = cfg.aliases.get(first) {
            let mut new_args: Vec<String> = alias_target
                .split_whitespace()
                .map(String::from)
                .collect();
            new_args.extend_from_slice(&args[1..]);
            return passthrough(&new_args);
        }
    }

    let status = Command::new(git_exe())
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| "Failed to execute git")?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

// ─── Current Branch / Repo Helpers ───────────────────────────────────────────

pub fn current_branch() -> Result<String> {
    git_output(&["rev-parse", "--abbrev-ref", "HEAD"])
}

pub fn repo_root() -> Result<String> {
    git_output(&["rev-parse", "--show-toplevel"])
}

pub fn default_branch() -> String {
    let cfg = config::load().unwrap_or_default();
    // Try to detect from remote HEAD
    let detected = git_output_lossy(&["symbolic-ref", "refs/remotes/origin/HEAD"]);
    if !detected.is_empty() {
        if let Some(branch) = detected.split('/').last() {
            return branch.to_string();
        }
    }
    cfg.general.default_branch
}

pub fn is_inside_git_repo() -> bool {
    Command::new(git_exe())
        .args(["rev-parse", "--git-dir"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ─── Enhanced Log ─────────────────────────────────────────────────────────────

/// Parse and pretty-print git log with beautiful colors
pub fn enhanced_log(extra_args: &[String]) -> Result<()> {
    let cfg = config::load().unwrap_or_default();

    // Separator we use between field in our format
    const SEP: &str = "\x01";
    const REC: &str = "\x02";

    // Build format string: record_sep + hash + sep + short_hash + sep + subject + sep + author_name + sep + rel_date + sep + refs + record_sep
    let fmt = format!(
        "{}%H{}%h{}%s{}%an{}%ar{}%D{}",
        REC, SEP, SEP, SEP, SEP, SEP, REC
    );

    let mut args = vec![
        "log".to_string(),
        format!("--pretty=format:{}", fmt),
    ];

    let has_graph = cfg.ui.show_graph && !extra_args.contains(&"--no-graph".to_string());
    if has_graph && !extra_args.iter().any(|a| a == "--graph" || a == "--no-graph") {
        args.push("--graph".to_string());
    }

    // Default limit unless user passed -n or --max-count
    let has_limit = extra_args.iter().any(|a| a.starts_with("-n") || a.starts_with("--max-count") || a.starts_with("--all"));
    if !has_limit {
        args.push(format!("-n{}", cfg.ui.log_limit));
    }

    args.extend_from_slice(extra_args);

    let output = git_output_lossy(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>());

    if output.is_empty() {
        println!("  {}", "No commits found.".bright_black());
        return Ok(());
    }

    println!(); // top padding

    for line in output.lines() {
        // Check if this line contains a commit record
        if let (Some(start), Some(end)) = (line.find('\x02'), line.rfind('\x02')) {
            if start != end {
                let record = &line[start + 1..end];
                let graph_prefix = &line[..start];
                let fields: Vec<&str> = record.splitn(7, '\x01').collect();

                if fields.len() >= 6 {
                    let _hash = fields[0];
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

                    println!("{}", entry.render(55));
                    continue;
                }
            }
        }

        // Graph-only lines (no commit data)
        if !line.trim().is_empty() {
            println!("{}", ui::colorize_graph(line));
        }
    }

    println!(); // bottom padding
    Ok(())
}

// ─── Enhanced Status ─────────────────────────────────────────────────────────

pub fn enhanced_status(_extra_args: &[String]) -> Result<()> {
    let branch = current_branch().unwrap_or_else(|_| "unknown".into());

    // Get porcelain v2 output
    let raw = git_output_lossy(&["status", "--porcelain=v2", "--branch", "--ahead-behind"]);

    let mut ahead: usize = 0;
    let mut behind: usize = 0;
    let mut upstream: Option<String> = None;

    let mut staged: Vec<(String, String)> = vec![];
    let mut unstaged: Vec<(String, String)> = vec![];
    let mut untracked: Vec<String> = vec![];
    let mut unmerged: Vec<(String, String)> = vec![];

    for line in raw.lines() {
        if line.starts_with("# branch.head ") {
            // skip, we already have it
        } else if line.starts_with("# branch.upstream ") {
            upstream = Some(line["# branch.upstream ".len()..].to_string());
        } else if line.starts_with("# branch.ab ") {
            let ab = &line["# branch.ab ".len()..];
            let parts: Vec<&str> = ab.split_whitespace().collect();
            if parts.len() >= 2 {
                ahead = parts[0].trim_start_matches('+').parse().unwrap_or(0);
                behind = parts[1].trim_start_matches('-').parse().unwrap_or(0);
            }
        } else if let Some(rest) = line.strip_prefix("1 ") {
            // Ordinary changed entry: "1 XY sub mH mI mW hH hI path"
            let xy = &rest[..2];
            let _path_start = rest.find('\t').map(|i| i + 1).unwrap_or(10);
            let fields: Vec<&str> = rest.splitn(9, ' ').collect();
            let path = if fields.len() >= 9 { fields[8] } else { rest.splitn(9, ' ').last().unwrap_or("") };
            let x = &xy[0..1]; // staged
            let y = &xy[1..2]; // unstaged
            if x != "." {
                staged.push((x.to_string(), path.to_string()));
            }
            if y != "." {
                unstaged.push((y.to_string(), path.to_string()));
            }
        } else if let Some(rest) = line.strip_prefix("2 ") {
            // Renamed/copied
            let xy = &rest[..2];
            let fields: Vec<&str> = rest.splitn(10, ' ').collect();
            let paths = if fields.len() >= 10 {
                let p = fields[9];
                if p.contains('\t') {
                    p.splitn(2, '\t').next().unwrap_or(p).to_string()
                } else {
                    p.to_string()
                }
            } else {
                rest[10..].to_string()
            };
            let x = &xy[0..1];
            let y = &xy[1..2];
            if x != "." {
                staged.push((x.to_string(), paths.clone()));
            }
            if y != "." {
                unstaged.push((y.to_string(), paths));
            }
        } else if let Some(rest) = line.strip_prefix("u ") {
            let fields: Vec<&str> = rest.splitn(12, ' ').collect();
            let path = fields.last().copied().unwrap_or("").to_string();
            unmerged.push((rest[..2].to_string(), path));
        } else if let Some(rest) = line.strip_prefix("? ") {
            untracked.push(rest.to_string());
        }
    }

    // ─── Print ────────────────────────────────────────────────────────────────

    // Branch header
    println!();
    print!(
        "  {} {}",
        "On branch".bright_black(),
        branch.green().bold()
    );
    if let Some(up) = &upstream {
        print!("  {}", format!("tracking {}", up).bright_black());
    }
    println!();

    // Ahead/behind
    if ahead > 0 || behind > 0 {
        println!("  {}", ui::format_ahead_behind(ahead, behind));
    }

    // Nothing to show
    if staged.is_empty() && unstaged.is_empty() && untracked.is_empty() && unmerged.is_empty() {
        println!();
        println!("  {} {}", "✓".green().bold(), "Working tree is clean".green());
        println!();
        return Ok(());
    }

    // Unmerged
    if !unmerged.is_empty() {
        ui::print_section("Conflicts", Some(unmerged.len()));
        for (code, path) in &unmerged {
            let (icon, _) = ui::status_icon("U");
            println!("  {} {} {}", "  ⚡".red().bold(), icon, path.red().bold());
        }
    }

    // Staged
    if !staged.is_empty() {
        ui::print_section("Staged Changes", Some(staged.len()));
        let last = staged.len() - 1;
        for (i, (code, path)) in staged.iter().enumerate() {
            let connector = if i == last { "└" } else { "├" }.bright_black();
            let (icon, code_colored) = ui::status_icon(code);
            println!("  {} {} {} {}", connector, code_colored, icon, path.green());
        }
    }

    // Unstaged
    if !unstaged.is_empty() {
        ui::print_section("Unstaged Changes", Some(unstaged.len()));
        let last = unstaged.len() - 1;
        for (i, (code, path)) in unstaged.iter().enumerate() {
            let connector = if i == last { "└" } else { "├" }.bright_black();
            let (icon, code_colored) = ui::status_icon(code);
            println!("  {} {} {} {}", connector, code_colored, icon, path.yellow());
        }
    }

    // Untracked
    if !untracked.is_empty() {
        ui::print_section("Untracked Files", Some(untracked.len()));
        let last = untracked.len() - 1;
        for (i, path) in untracked.iter().enumerate() {
            let connector = if i == last { "└" } else { "├" }.bright_black();
            println!("  {} {} {}", connector, "?".bright_black(), path.bright_black());
        }
    }

    println!();

    // Hints
    if !staged.is_empty() {
        println!(
            "  {}  {}",
            "tip:".bright_black(),
            "vcli commit  — commit staged changes".bright_black()
        );
    } else if !unstaged.is_empty() || !untracked.is_empty() {
        println!(
            "  {}  {}",
            "tip:".bright_black(),
            "git add <file>  or  git add -A  to stage".bright_black()
        );
    }
    println!();

    Ok(())
}

// ─── Enhanced Diff ────────────────────────────────────────────────────────────

pub fn enhanced_diff(extra_args: &[String]) -> Result<()> {
    let cfg = config::load().unwrap_or_default();
    let tool = resolve_diff_tool(&cfg.diff.tool);

    match tool.as_str() {
        "delta" => {
            if which::which("delta").is_ok() {
                let output = Command::new(git_exe())
                    .args(["diff"])
                    .args(extra_args)
                    .stdout(Stdio::piped())
                    .spawn()?
                    .stdout
                    .context("no stdout")?;

                let status = Command::new("delta")
                    .stdin(output)
                    .status()?;
                if !status.success() {
                    // fall through to builtin
                }
                return Ok(());
            }
            passthrough_with_subcommand("diff", extra_args)
        }
        "diff-so-fancy" => {
            if which::which("diff-so-fancy").is_ok() {
                let output = Command::new(git_exe())
                    .args(["diff", "--color=always"])
                    .args(extra_args)
                    .stdout(Stdio::piped())
                    .spawn()?
                    .stdout
                    .context("no stdout")?;

                Command::new("diff-so-fancy")
                    .stdin(output)
                    .status()?;
                return Ok(());
            }
            passthrough_with_subcommand("diff", extra_args)
        }
        _ => passthrough_with_subcommand("diff", extra_args),
    }
}

fn resolve_diff_tool(tool: &str) -> String {
    match tool {
        "auto" => {
            if which::which("delta").is_ok() {
                "delta".to_string()
            } else if which::which("diff-so-fancy").is_ok() {
                "diff-so-fancy".to_string()
            } else {
                "builtin".to_string()
            }
        }
        other => other.to_string(),
    }
}

fn passthrough_with_subcommand(sub: &str, extra: &[String]) -> Result<()> {
    let mut args = vec![sub.to_string()];
    args.extend_from_slice(extra);
    passthrough(&args)
}

// ─── Enhanced Branch ─────────────────────────────────────────────────────────

pub fn enhanced_branch(extra_args: &[String]) -> Result<()> {
    // If extra args look like modifications (create/delete), just pass through
    let mutating = extra_args.iter().any(|a| {
        a == "-d" || a == "-D" || a == "--delete" || a == "-m" || a == "--move"
            || a == "--copy" || a == "-c"
    });

    if mutating || !extra_args.is_empty() && !extra_args[0].starts_with('-') {
        // Creating a new branch or mutating: pass through
        let mut args = vec!["branch".to_string()];
        args.extend_from_slice(extra_args);
        return passthrough(&args);
    }

    // List branches with metadata
    let raw = git_output_lossy(&[
        "branch",
        "--format=%(refname:short)\t%(objectname:short)\t%(subject)\t%(authorname)\t%(committerdate:relative)\t%(upstream:short)\t%(HEAD)",
        "-a",
    ]);

    let current = current_branch().unwrap_or_default();

    println!();
    let mut table = ui::Table::new(vec!["", "Branch", "Hash", "Last Commit", "Author", "Date", "Tracking"]);

    for line in raw.lines() {
        let fields: Vec<&str> = line.splitn(7, '\t').collect();
        if fields.len() < 7 {
            continue;
        }
        let (name, hash, subject, author, date, upstream, head_marker) =
            (fields[0], fields[1], fields[2], fields[3], fields[4], fields[5], fields[6]);

        // Skip remote tracking branches in the list (they're shown as "remote/branch")
        let is_remote = name.starts_with("remotes/");
        let display_name = if is_remote {
            name.trim_start_matches("remotes/").to_string()
        } else {
            name.to_string()
        };

        let marker = if head_marker == "*" { "◉" } else if is_remote { "○" } else { "◯" };
        let marker_colored = if head_marker == "*" {
            marker.green().bold().to_string()
        } else if is_remote {
            marker.red().dimmed().to_string()
        } else {
            marker.bright_black().to_string()
        };

        let branch_colored = if head_marker == "*" {
            display_name.green().bold().to_string()
        } else if is_remote {
            display_name.red().to_string()
        } else {
            display_name.white().to_string()
        };

        let subj = if subject.len() > 40 {
            format!("{}…", &subject[..39])
        } else {
            subject.to_string()
        };

        table.add_row(vec![
            marker_colored,
            branch_colored,
            ui::color_hash(hash),
            ui::color_subject(&subj),
            ui::color_author(&if author.len() > 18 { format!("{}…", &author[..17]) } else { author.to_string() }),
            ui::color_date(date),
            if upstream.is_empty() { "—".bright_black().to_string() } else { ui::color_branch(upstream) },
        ]);
    }

    table.print();
    println!();
    Ok(())
}

// ─── Enhanced Show ────────────────────────────────────────────────────────────

pub fn enhanced_show(extra_args: &[String]) -> Result<()> {
    let cfg = config::load().unwrap_or_default();
    let tool = resolve_diff_tool(&cfg.diff.tool);

    // Show commit metadata beautifully, then the diff
    let rev = extra_args.first().map(|s| s.as_str()).unwrap_or("HEAD");

    let meta_fmt = "%H\x01%h\x01%s\x01%b\x01%an\x01%ae\x01%ai\x01%ar\x01%D\x01%P";
    let meta_raw = git_output_lossy(&["show", "-s", &format!("--format={}", meta_fmt), rev]);

    for line in meta_raw.lines() {
        let fields: Vec<&str> = line.splitn(10, '\x01').collect();
        if fields.len() >= 9 {
            let (hash, short_hash, subject, body, author, email, date_iso, date_rel, refs) =
                (fields[0], fields[1], fields[2], fields[3], fields[4], fields[5], fields[6], fields[7], fields[8]);

            println!();
            println!("  {} {}{}", "commit".bright_black(), ui::color_hash(hash), ui::format_refs(refs));
            println!("  {} {}", "Author:".bright_black(), format!("{} <{}>", author, email).cyan());
            println!("  {}   {} {}", "Date:".bright_black(), date_iso.bright_black(), format!("({})", date_rel).bright_black());
            println!();
            println!("      {}", ui::color_subject(subject).bold());
            if !body.trim().is_empty() {
                println!();
                for line in body.lines() {
                    println!("      {}", line.white());
                }
            }
            println!();
            break;
        }
    }

    // Now show the diff
    let diff_args: Vec<String> = {
        let mut a = vec!["-1".to_string(), rev.to_string()];
        // Remove the rev from extra_args if present
        a.extend(extra_args.iter().filter(|&s| s != rev).cloned());
        a
    };
    enhanced_diff(&diff_args)
}
