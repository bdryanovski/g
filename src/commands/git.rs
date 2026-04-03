//! Git command helpers and enhanced output modes.
//!
//! ## Tutorial overview
//!
//! This module is the "engine room" of the CLI.  It handles all interaction
//! with the underlying `git` binary.  It provides:
//!
//! - **Low-level wrappers** (`git_output`, `git_output_lossy`, `passthrough`)
//!   for capturing or streaming git output.
//! - **Repo helpers** (`current_branch`, `repo_root`, `default_branch`) used
//!   throughout the codebase.
//! - **Enhanced commands**: colourised, opinionated replacements for `log`,
//!   `status`, `diff`, `branch`, and `show`.
//! - **Dry-run mode**: an atomic flag that, when set, prints the git commands
//!   that *would* run instead of executing them.
//!
//! ## Rust concepts used here
//!
//! - `std::process::Command` for spawning and interacting with external processes.
//! - `AtomicBool` and `AtomicUsize` for thread-safe, lock-free global state.
//! - `String::from_utf8_lossy` to safely decode potentially non-UTF-8 output.
//! - `match` and `if let` for robust error handling and optional-value extraction.
//! - `static` variables for program-wide flags that need to persist across calls.

use anyhow::{bail, Context, Result};

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::config;
use crate::ui;

// ─── Dry-run state ────────────────────────────────────────────────────────────

// `static` variables live for the entire lifetime of the program.
// `AtomicBool` / `AtomicUsize` are safe to access from multiple threads without
// a `Mutex` because they use hardware-level atomic operations.
static DRY_RUN: AtomicBool = AtomicBool::new(false);
static DRY_RUN_STEP: AtomicUsize = AtomicUsize::new(0);

/// Enable or disable dry-run mode and reset the step counter.
///
/// When dry-run is enabled, [`git_mutate`] prints the planned command instead
/// of executing it.
pub fn set_dry_run(enabled: bool) {
    DRY_RUN.store(enabled, Ordering::SeqCst);
    DRY_RUN_STEP.store(0, Ordering::SeqCst);
}

/// Returns `true` when dry-run mode is active.
///
/// Use this to skip side-effects (e.g. writing files) that should not happen
/// during a preview run.
pub fn is_dry_run() -> bool {
    DRY_RUN.load(Ordering::SeqCst)
}

/// Returns the number of mutating steps logged so far in dry-run mode.
pub fn step_count() -> usize {
    DRY_RUN_STEP.load(Ordering::SeqCst)
}

/// Increments the step counter and returns the new value.
fn next_step() -> usize {
    DRY_RUN_STEP.fetch_add(1, Ordering::SeqCst) + 1
}

/// Execute a mutating git command, or print the planned command in dry-run mode.
///
/// Use this for any `git` invocation that writes to the repository (checkout,
/// commit, rebase, …).  Read-only calls (log, rev-parse, …) should use
/// [`git_output`] directly.
///
/// In dry-run mode this function prints a numbered step and returns `Ok("")`
/// without touching the repository.
///
/// # Errors
///
/// Propagates any error from [`git_output`] when not in dry-run mode.
pub fn git_mutate(args: &[&str], explanation: &str) -> Result<String> {
    if is_dry_run() {
        print_dry_run_git(args, explanation);
        return Ok(String::new());
    }
    git_output(args)
}

/// Log a non-git side effect (file write, API call, …) in dry-run mode.
///
/// In normal (non-dry-run) mode this is a no-op.
pub fn dry_run_action(action: &str, explanation: &str) {
    if is_dry_run() {
        let step = next_step();
        let label = format!("Step {}", step);
        ui::print_blank();
        ui::print_indented(&format!(
            "{} {} {}",
            ui::primary_bold(&label),
            ui::muted("▸"),
            ui::warning(action)
        ));
        ui::print_indented(&format!(
            "{}  {}",
            " ".repeat(label.len()),
            ui::muted(explanation)
        ));
    }
}

/// Print a dry-run step for a git command.
///
/// Delegates to [`dry_run_action`] after formatting the command as
/// `"git <args>"`.  This removes the ~12 lines of duplicated output logic
/// that previously existed between the two functions.
fn print_dry_run_git(args: &[&str], explanation: &str) {
    dry_run_action(&format!("git {}", args.join(" ")), explanation);
}

/// Print the dry-run banner shown at the start of a `--dry-run` invocation.
pub fn dry_run_banner() {
    ui::print_blank();
    ui::print_fieldset("⚡  Dry Run — preview only, no changes will be made");
}

/// Print the dry-run footer shown at the end of a `--dry-run` invocation.
///
/// Summarises the number of operations that would be performed.
pub fn dry_run_footer() {
    let steps = step_count();
    ui::print_blank();
    if steps > 0 {
        ui::print_fieldset(&format!(
            "{}  {} would be performed — re-run without --dry-run to execute",
            steps,
            if steps == 1 {
                "operation"
            } else {
                "operations"
            }
        ));
    } else {
        ui::print_fieldset("No mutating operations to preview");
    }
    ui::print_blank();
}

// ─── Git executable ───────────────────────────────────────────────────────────

/// Resolve the git executable path from config, falling back to `"git"`.
///
/// Reads `[general].git_path` from the user config.  If the config cannot be
/// loaded or the key is absent, `"git"` is returned so the OS resolves it via
/// `$PATH`.
pub fn git_exe() -> String {
    let cfg = config::load().unwrap_or_default();
    cfg.general.git_path.unwrap_or_else(|| "git".to_string())
}

/// Run `git` with `args` and return stdout as a trimmed `String`.
///
/// Stderr from git is captured and returned as the error message on non-zero
/// exit so callers get the same diagnostic git would normally print.
///
/// # Errors
///
/// Returns an error if:
/// - The git process cannot be spawned (e.g. `git` not found in `$PATH`).
/// - git exits with a non-zero status; the error contains the captured stderr.
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

/// Returns `true` if `ancestor` is reachable from `descendant`'s history.
///
/// Internally runs `git merge-base --is-ancestor <ancestor> <descendant>`.
///
/// # Errors
///
/// Returns an error if the git process cannot be spawned or exits with a code
/// other than `0` (ancestor) or `1` (not an ancestor).
pub fn is_ancestor(ancestor: &str, descendant: &str) -> Result<bool> {
    let status = Command::new(git_exe())
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .status()
        .with_context(|| {
            format!(
                "Failed to run git merge-base --is-ancestor {} {}",
                ancestor, descendant
            )
        })?;
    if status.success() {
        Ok(true)
    } else if status.code() == Some(1) {
        Ok(false)
    } else {
        bail!("git merge-base --is-ancestor exited with {:?}", status);
    }
}

/// Returns `true` when `git status --porcelain` produces no output (clean tree).
///
/// # Errors
///
/// Returns an error if the git process cannot be spawned or exits non-zero.
pub fn working_tree_clean() -> Result<bool> {
    let s = git_output(&["status", "--porcelain"])?;
    Ok(s.is_empty())
}

/// Bail with a standardised message if the working tree has uncommitted changes.
///
/// Three commands (`branch squash`, `stack squash`, `stack fold`) all need
/// to check the working tree before doing history-rewriting operations.
/// This helper gives them one consistent message and removes the repeated
/// `if !working_tree_clean()? { bail!(…) }` block.
///
/// `operation` is the verb used in the message, e.g. `"squashing"` or `"folding"`.
///
/// # Errors
///
/// Returns an error immediately when the tree is dirty.  When the tree is
/// clean the function returns `Ok(())` and the caller continues normally.
pub fn require_clean_tree(operation: &str) -> Result<()> {
    if !working_tree_clean()? {
        bail!(
            "Working tree is not clean. Commit or stash changes before {}.",
            operation
        );
    }
    Ok(())
}

/// Run `git` with `args` and return stdout as a `String`, ignoring a non-zero exit.
///
/// Non-UTF-8 bytes in the output are replaced with the Unicode replacement
/// character (`U+FFFD`).  Use this for display-only calls where a git error
/// (e.g. "no commits yet") should silently produce an empty string.
pub fn git_output_lossy(args: &[&str]) -> String {
    Command::new(git_exe())
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Stream a git invocation directly to the terminal (stdin/stdout/stderr inherited).
///
/// Used for "passthrough" commands where we want git's own interactive output,
/// pager, colour handling, etc.  If git exits non-zero, this function calls
/// [`std::process::exit`] with that code so the shell receives the correct
/// exit status.
///
/// In dry-run mode the command is printed but not executed.
///
/// # Errors
///
/// Returns an error if the git process cannot be spawned.
pub fn passthrough(args: &[String]) -> Result<()> {
    let cfg = config::load().unwrap_or_default();

    // Check aliases first so `g co` works even as a passthrough.
    if let Some(first) = args.first() {
        if let Some(alias_target) = cfg.aliases.get(first) {
            let mut new_args: Vec<String> =
                alias_target.split_whitespace().map(String::from).collect();
            new_args.extend_from_slice(&args[1..]);
            return passthrough(&new_args);
        }
    }

    if is_dry_run() {
        let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        print_dry_run_git(&str_args, "Passthrough — forwarded to git as-is");
        return Ok(());
    }

    let status = Command::new(git_exe())
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("Failed to execute git")?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

// ─── Repo helpers ─────────────────────────────────────────────────────────────

/// Return the name of the currently checked-out branch.
///
/// Returns `"HEAD"` when in detached-HEAD state.
///
/// # Errors
///
/// Returns an error if `git rev-parse --abbrev-ref HEAD` fails (e.g. no
/// commits in the repo yet).
pub fn current_branch() -> Result<String> {
    git_output(&["rev-parse", "--abbrev-ref", "HEAD"])
}

/// Return the absolute path of the repository root directory.
///
/// # Errors
///
/// Returns an error if the command is run outside a git repository.
pub fn repo_root() -> Result<String> {
    git_output(&["rev-parse", "--show-toplevel"])
}

/// Determine the default branch name using `origin/HEAD`, falling back to config.
///
/// First tries `git symbolic-ref refs/remotes/origin/HEAD` to detect what the
/// remote considers its default.  Falls back to `config.general.default_branch`
/// (typically `"main"`) when no remote HEAD is set.
pub fn default_branch() -> String {
    let cfg = config::load().unwrap_or_default();
    let detected = git_output_lossy(&["symbolic-ref", "refs/remotes/origin/HEAD"]);
    if !detected.is_empty() {
        if let Some(branch) = detected.split('/').next_back() {
            return branch.to_string();
        }
    }
    cfg.general.default_branch
}

/// Returns `true` if the current directory is inside a git repository.
///
/// Both stdout and stderr are suppressed so nothing is printed regardless of
/// the result.
pub fn is_inside_git_repo() -> bool {
    Command::new(git_exe())
        .args(["rev-parse", "--git-dir"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ─── Enhanced log ─────────────────────────────────────────────────────────────

/// Parse and pretty-print `git log` with colours, graph art, and aligned columns.
///
/// The implementation works in three phases:
///
/// 1. Build a custom `--pretty=format:` string using ASCII control characters
///    (`\x01` as field separator, `\x02` as record separator) to reliably
///    distinguish fields even when subjects contain special characters.
/// 2. Append `--graph` and any user-supplied extra args, then run git.
/// 3. Parse each output line: lines containing a `\x02`-delimited record are
///    split into fields and rendered via [`ui::CommitEntry`]; lines containing
///    only graph art are colourised with [`ui::colorize_graph`].
///
/// # Errors
///
/// Returns an error if the config cannot be loaded.
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

// ─── Enhanced status ─────────────────────────────────────────────────────────

/// Pretty-print git status using `--porcelain=v2` machine-readable output.
///
/// Shows staged, unstaged, untracked, and conflicted files in separate sections
/// with colour-coded status codes and Unicode icons, along with ahead/behind
/// counts for the current tracking branch.
///
/// # Errors
///
/// Returns an error if the config cannot be loaded.
pub fn enhanced_status(_extra_args: &[String]) -> Result<()> {
    let branch = current_branch().unwrap_or_else(|_| "unknown".into());

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
            // Already captured in `current_branch()` above.
        } else if let Some(up) = line.strip_prefix("# branch.upstream ") {
            upstream = Some(up.to_string());
        } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
            // "# branch.ab +3 -1" — ahead/behind counts.
            let parts: Vec<&str> = ab.split_whitespace().collect();
            if parts.len() >= 2 {
                ahead = parts[0].trim_start_matches('+').parse().unwrap_or(0);
                behind = parts[1].trim_start_matches('-').parse().unwrap_or(0);
            }
        } else if let Some(rest) = line.strip_prefix("1 ") {
            // Ordinary changed file: "1 XY sub mH mI mW hH hI path"
            let xy = &rest[..2];
            let fields: Vec<&str> = rest.splitn(9, ' ').collect();
            let path = if fields.len() >= 9 {
                fields[8]
            } else {
                rest.splitn(9, ' ').last().unwrap_or("")
            };
            let x = &xy[0..1]; // staged status
            let y = &xy[1..2]; // unstaged status
            if x != "." {
                staged.push((x.to_string(), path.to_string()));
            }
            if y != "." {
                unstaged.push((y.to_string(), path.to_string()));
            }
        } else if let Some(rest) = line.strip_prefix("2 ") {
            // Renamed or copied file.
            let xy = &rest[..2];
            let fields: Vec<&str> = rest.splitn(10, ' ').collect();
            let paths = if fields.len() >= 10 {
                let p = fields[9];
                if p.contains('\t') {
                    p.split('\t').next().unwrap_or(p).to_string()
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
            // Unmerged (conflict) entry.
            let fields: Vec<&str> = rest.splitn(12, ' ').collect();
            let path = fields.last().copied().unwrap_or("").to_string();
            unmerged.push((rest[..2].to_string(), path));
        } else if let Some(rest) = line.strip_prefix("? ") {
            // Untracked file.
            untracked.push(rest.to_string());
        }
    }

    // ─── Print output ─────────────────────────────────────────────────────────

    ui::print_blank();
    print!("  {} {}", ui::muted("On branch"), ui::success_bold(&branch));
    if let Some(up) = &upstream {
        print!("  {}", ui::muted(&format!("tracking {}", up)));
    }
    ui::print_blank();

    if ahead > 0 || behind > 0 {
        ui::print_indented(&ui::format_ahead_behind(ahead, behind));
    }

    if staged.is_empty() && unstaged.is_empty() && untracked.is_empty() && unmerged.is_empty() {
        ui::print_blank();
        ui::print_indented(&format!(
            "{} {}",
            ui::success_bold("✓"),
            ui::success("Working tree is clean")
        ));
        ui::print_blank();
        return Ok(());
    }

    if !unmerged.is_empty() {
        ui::print_section("Conflicts", Some(unmerged.len()));
        for (code, path) in &unmerged {
            let (icon, _) = ui::status_icon(code);
            ui::print_indented(&format!(
                "{} {} {}",
                ui::danger_bold("  ⚡"),
                icon,
                ui::danger_bold(path)
            ));
        }
    }

    if !staged.is_empty() {
        ui::print_section("Staged Changes", Some(staged.len()));
        let last = staged.len() - 1;
        for (i, (code, path)) in staged.iter().enumerate() {
            let connector = ui::muted(if i == last { "└" } else { "├" });
            let (icon, code_colored) = ui::status_icon(code);
            ui::print_indented(&format!(
                "{} {} {} {}",
                connector,
                code_colored,
                icon,
                ui::success(path)
            ));
        }
    }

    if !unstaged.is_empty() {
        ui::print_section("Unstaged Changes", Some(unstaged.len()));
        let last = unstaged.len() - 1;
        for (i, (code, path)) in unstaged.iter().enumerate() {
            let connector = ui::muted(if i == last { "└" } else { "├" });
            let (icon, code_colored) = ui::status_icon(code);
            ui::print_indented(&format!(
                "{} {} {} {}",
                connector,
                code_colored,
                icon,
                ui::warning(path)
            ));
        }
    }

    if !untracked.is_empty() {
        ui::print_section("Untracked Files", Some(untracked.len()));
        let last = untracked.len() - 1;
        for (i, path) in untracked.iter().enumerate() {
            let connector = ui::muted(if i == last { "└" } else { "├" });
            ui::print_indented(&format!(
                "{} {} {}",
                connector,
                ui::muted("?"),
                ui::muted(path)
            ));
        }
    }

    ui::print_blank();

    if !staged.is_empty() {
        ui::print_tip(&format!(
            "{}  commit staged changes",
            ui::warning(&format!("{} commit", crate::bin_name()))
        ));
    } else if !unstaged.is_empty() || !untracked.is_empty() {
        ui::print_tip("git add <file>  or  git add -A  to stage");
    }
    ui::print_blank();

    Ok(())
}

// ─── Interactive add ──────────────────────────────────────────────────────────

/// Present full-screen ratatui pickers to stage and/or unstage files.
///
/// Two sequential screens are shown (each only when relevant):
///
/// 1. **Unstage picker** — shows files that are staged (index column X is
///    non-blank); selecting them runs `git restore --staged <file>`.
/// 2. **Stage picker** — shows untracked and working-tree-modified files
///    (column Y is non-blank); selecting them runs `git add <file>`.
///
/// Called by `g add` when no path arguments are supplied.
///
/// # Errors
///
/// Returns an error if the current directory is not a git repo or if any
/// git command fails.
pub fn interactive_add() -> Result<()> {
    if !is_inside_git_repo() {
        bail!("Not inside a git repository.");
    }

    if is_dry_run() {
        dry_run_action(
            "git add / restore --staged <interactive>",
            "Launch interactive picker to stage or unstage files",
        );
        return Ok(());
    }

    // Fetch raw porcelain output without trimming (the leading space on the first
    // line carries the index status and must be preserved).
    let raw_out = Command::new(git_exe())
        .args(["status", "--porcelain"])
        .output()
        .context("Failed to run `git status --porcelain`")?;

    let raw = String::from_utf8_lossy(&raw_out.stdout);

    if raw.trim().is_empty() {
        ui::print_blank();
        ui::print_info("Nothing to do — working tree is clean.");
        ui::print_blank();
        return Ok(());
    }

    // Parsed file entry.
    struct FileEntry {
        /// Index status character (column 1 in porcelain).
        x: String,
        /// Working-tree status character (column 2 in porcelain).
        y: String,
        path: String,
    }

    let mut staged: Vec<FileEntry> = Vec::new(); // X != ' '/'?' — can unstage
    let mut stageable: Vec<FileEntry> = Vec::new(); // Y != ' '/'?' — can stage

    for line in raw.lines() {
        if line.len() < 3 {
            continue;
        }
        let x = line[0..1].to_string();
        let y = line[1..2].to_string();
        let path = unquote_path(line[3..].trim());

        // Skip ignored files.
        if x == "!" && y == "!" {
            continue;
        }

        // Staged changes (index modified — can unstage).
        if x != " " && x != "?" && x != "!" {
            staged.push(FileEntry {
                x: x.clone(),
                y: y.clone(),
                path: path.clone(),
            });
        }

        // Stageable changes: untracked OR working-tree modified/deleted.
        if (x == "?" && y == "?") || (y != " " && y != "?" && y != "!") {
            stageable.push(FileEntry { x, y, path });
        }
    }

    let mut any_action = false;

    // ── Step 1: Unstage picker ────────────────────────────────────────────────
    if !staged.is_empty() {
        let staged_paths: Vec<String> = staged.iter().map(|e| e.path.clone()).collect();
        let staged_opts: Vec<ui::SelectOption> = staged
            .iter()
            .map(|e| {
                let (icon, _) = ui::status_icon(&e.x);
                ui::SelectOption::with_description(
                    format!("{}  {}  {}", e.x, icon, e.path),
                    "(staged — select to unstage)",
                )
            })
            .collect();

        let to_unstage = ui::multi_select("Unstage Files", &staged_opts);
        ui::print_blank();

        if !to_unstage.is_empty() {
            let paths: Vec<&str> = to_unstage
                .iter()
                .map(|&i| staged_paths[i].as_str())
                .collect();
            let count = paths.len();
            let pb = ui::spinner(&format!(
                "Unstaging {} file{}…",
                count,
                if count == 1 { "" } else { "s" }
            ));

            let mut git_args = vec!["restore", "--staged", "--"];
            git_args.extend(paths.iter().copied());

            match git_output(&git_args) {
                Ok(_) => ui::spinner_success(
                    pb,
                    &format!(
                        "Unstaged {}  {}",
                        ui::warning_bold(&count.to_string()),
                        if count == 1 { "file" } else { "files" }
                    ),
                ),
                Err(e) => ui::spinner_error(pb, &format!("Failed to unstage: {e}")),
            }
            ui::print_blank();
            any_action = true;
        }
    }

    // ── Step 2: Stage picker ──────────────────────────────────────────────────
    if !stageable.is_empty() {
        let stageable_paths: Vec<String> = stageable.iter().map(|e| e.path.clone()).collect();
        let stageable_opts: Vec<ui::SelectOption> = stageable
            .iter()
            .map(|e| {
                let (icon, _) = ui::status_icon(&e.y);
                let label = if e.x == "?" && e.y == "?" {
                    format!("?  {}", e.path)
                } else {
                    format!("{}  {}  {}", e.y, icon, e.path)
                };
                ui::SelectOption::new(label)
            })
            .collect();

        let to_stage = ui::multi_select("Stage Files", &stageable_opts);
        ui::print_blank();

        if !to_stage.is_empty() {
            let paths: Vec<&str> = to_stage
                .iter()
                .map(|&i| stageable_paths[i].as_str())
                .collect();
            let count = paths.len();
            let pb = ui::spinner(&format!(
                "Staging {} file{}…",
                count,
                if count == 1 { "" } else { "s" }
            ));

            let mut git_args = vec!["add", "--"];
            git_args.extend(paths.iter().copied());

            match git_output(&git_args) {
                Ok(_) => {
                    ui::spinner_success(
                        pb,
                        &format!(
                            "Staged {}  {}",
                            ui::warning_bold(&count.to_string()),
                            if count == 1 { "file" } else { "files" }
                        ),
                    );
                    ui::print_tip(&format!(
                        "{}  commit staged changes",
                        ui::warning(&format!("{} commit", crate::bin_name()))
                    ));
                }
                Err(e) => ui::spinner_error(pb, &format!("Failed to stage: {e}")),
            }
            ui::print_blank();
            any_action = true;
        }
    }

    if !any_action {
        ui::print_info("No files selected — nothing changed.");
        ui::print_blank();
    }

    Ok(())
}

/// Strip git's double-quote wrapping from a path, if present.
///
/// `git status --porcelain` quotes paths that contain special characters
/// (spaces, newlines, non-ASCII bytes) with double quotes.  This removes
/// the outer quotes so the path can be passed to `git add` as a plain string.
fn unquote_path(p: &str) -> String {
    if p.starts_with('"') && p.ends_with('"') && p.len() >= 2 {
        p[1..p.len() - 1].to_string()
    } else {
        p.to_string()
    }
}

// ─── Enhanced diff ────────────────────────────────────────────────────────────

/// Run diff using a configured external tool if available, otherwise passthrough.
///
/// The tool is selected from `config.diff.tool`:
/// - `"auto"` → detect `delta` or `diff-so-fancy` in `$PATH`.
/// - `"delta"` / `"diff-so-fancy"` → pipe git diff output through the tool.
/// - Anything else → forward directly to `git diff`.
///
/// # Errors
///
/// Returns an error if the git diff process cannot be spawned, or if the config
/// cannot be loaded.
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

                Command::new("delta").stdin(output).status()?;
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

                Command::new("diff-so-fancy").stdin(output).status()?;
                return Ok(());
            }
            passthrough_with_subcommand("diff", extra_args)
        }
        _ => passthrough_with_subcommand("diff", extra_args),
    }
}

/// Determine which diff tool to use based on the config value and `$PATH`.
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

/// Prepend a subcommand name to `extra` and delegate to [`passthrough`].
fn passthrough_with_subcommand(sub: &str, extra: &[String]) -> Result<()> {
    let mut args = vec![sub.to_string()];
    args.extend_from_slice(extra);
    passthrough(&args)
}

// ─── Enhanced branch ─────────────────────────────────────────────────────────

/// Returns `true` if `refspec` resolves to an existing object in the repo.
fn git_ref_exists(refspec: &str) -> bool {
    Command::new(git_exe())
        .args(["rev-parse", "-q", "--verify", refspec])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Resolve the "mainline" ref used as the squash base when no `--base` is given.
///
/// Resolution order:
/// 1. The explicit `--base` value from the user.
/// 2. `@{upstream}` — the configured tracking branch.
/// 3. `origin/<default_branch>` — the remote default branch.
/// 4. `<default_branch>` — the local default branch.
///
/// # Errors
///
/// Returns an error if none of the candidates exist in the repository.
fn resolve_branch_squash_mainline(user_base: Option<&str>) -> Result<String> {
    if let Some(b) = user_base {
        git_output(&["rev-parse", "--verify", b])
            .with_context(|| format!("Base ref '{}' is not a valid object", b))?;
        return Ok(b.to_string());
    }
    if git_ref_exists("@{upstream}") {
        return Ok("@{upstream}".to_string());
    }
    let db = default_branch();
    let origin_db = format!("origin/{}", db);
    if git_ref_exists(&origin_db) {
        return Ok(origin_db);
    }
    if git_ref_exists(&db) {
        return Ok(db);
    }
    bail!(
        "Could not determine squash base. Pass --base <ref>, set upstream with \
         `git branch -u <remote>/<branch>`, or ensure `{}` or `{}` exists.",
        origin_db,
        db
    );
}

/// Resolve the commit message for a squash operation.
///
/// Priority:
/// - If `message` is `Some`, use it directly.
/// - Otherwise use the subject of the *oldest* commit in `range`.
/// - If that is empty (e.g. the range is empty), fall back to
///   `"Squash branch \`<branch>\`"`.
///
/// This logic was copy-pasted verbatim in both `branch_squash` (this file) and
/// `squash` in `stack.rs`.  Extracting it here gives both a single source of truth.
///
/// # Errors
///
/// Returns an error if the `git log` invocation fails.
pub fn resolve_squash_message(message: Option<&str>, range: &str, branch: &str) -> Result<String> {
    if let Some(m) = message {
        return Ok(m.to_string());
    }
    let oldest = git_output(&["log", range, "--reverse", "--format=%s", "-1"])?;
    if oldest.is_empty() {
        Ok(format!("Squash branch `{}`", branch))
    } else {
        Ok(oldest)
    }
}

/// Collapse all commits on the current branch into a single commit on top of
/// its merge-base with `base`.
///
/// Steps:
/// 1. Compute `git merge-base HEAD <base>`.
/// 2. `git reset --soft <merge-base>` to stage all branch changes at once.
/// 3. `git commit -m <message>` to create the single squashed commit.
///
/// # Errors
///
/// Returns an error if:
/// - The working tree is dirty.
/// - The current state is detached HEAD.
/// - The base ref does not exist.
/// - Any git operation fails.
pub fn branch_squash(message: Option<&str>, base: Option<&str>) -> Result<()> {
    require_clean_tree("squashing")?;
    let branch = current_branch()?;
    if branch == "HEAD" {
        bail!("Detached HEAD; checkout a branch first.");
    }
    let mainline = resolve_branch_squash_mainline(base)?;
    let fork = git_output(&["merge-base", "HEAD", &mainline]).with_context(|| {
        format!(
            "Could not compute merge-base with '{}'. Try a different --base.",
            mainline
        )
    })?;

    let range = format!("{}..HEAD", fork);
    let count: u32 = git_output(&["rev-list", "--count", &range])?
        .parse()
        .unwrap_or(0);
    if count == 0 {
        bail!(
            "No commits to squash on this branch relative to merge-base with '{}'.",
            mainline
        );
    }

    let commit_msg = resolve_squash_message(message, &range, &branch)?;

    let fork_short = git_output(&["rev-parse", "--short", &fork]).unwrap_or(fork.clone());

    ui::print_blank();
    ui::print_key_value_pairs(&[
        ("Squashing branch", ui::success_bold(&branch)),
        (
            "Merge-base with",
            format!("{} ({})", ui::primary(&mainline), ui::primary(&fork_short)),
        ),
    ]);
    ui::print_blank();

    git_mutate(
        &["reset", "--soft", &fork],
        &format!(
            "Soft-reset to merge-base with '{}' so all branch changes are staged once",
            mainline
        ),
    )?;

    git_mutate(
        &["commit", "-m", &commit_msg],
        "Create a single commit with the squashed changes",
    )
    .context("Failed to commit squashed changes")?;

    if !is_dry_run() {
        ui::print_blank();
        ui::print_success(&format!(
            "Squashed {} into one commit",
            ui::success_bold(&branch)
        ));
        ui::print_blank();
    }
    Ok(())
}

/// List branches with metadata and colour, or pass through for mutation flags.
///
/// When `extra_args` contains flags that create, delete, or move branches
/// (`-d`, `-D`, `-m`, `--move`, `--copy`, `-b`, `--create`), the call is
/// forwarded to `git branch` unchanged.
///
/// Otherwise a formatted table is printed showing branch name, hash, last
/// commit subject, author, date, and upstream tracking branch.
///
/// # Errors
///
/// Returns an error if the git command cannot be spawned.
pub fn enhanced_branch(extra_args: &[String]) -> Result<()> {
    let mutating = extra_args.iter().any(|a| {
        a == "-d"
            || a == "-D"
            || a == "--delete"
            || a == "-m"
            || a == "--move"
            || a == "--copy"
            || a == "-c"
            || a == "-b"
            || a == "--create"
    });

    if mutating || (!extra_args.is_empty() && !extra_args[0].starts_with('-')) {
        let mut args = vec!["branch".to_string()];
        args.extend_from_slice(extra_args);
        return passthrough(&args);
    }

    let raw = git_output_lossy(&[
        "branch",
        "--format=%(refname:short)\t%(objectname:short)\t%(subject)\t%(authorname)\t%(committerdate:relative)\t%(upstream:short)\t%(HEAD)",
        "-a",
    ]);

    ui::print_blank();
    let mut table = ui::Table::new(vec![
        "",
        "Branch",
        "Hash",
        "Last Commit",
        "Author",
        "Date",
        "Tracking",
    ]);

    for line in raw.lines() {
        let fields: Vec<&str> = line.splitn(7, '\t').collect();
        if fields.len() < 7 {
            continue;
        }
        let (name, hash, subject, author, date, upstream, head_marker) = (
            fields[0], fields[1], fields[2], fields[3], fields[4], fields[5], fields[6],
        );

        // Remote branches are prefixed with "remotes/" in the ref format.
        let is_remote = name.starts_with("remotes/");
        let display_name = if is_remote {
            name.trim_start_matches("remotes/").to_string()
        } else {
            name.to_string()
        };

        let marker = if head_marker == "*" {
            "◉"
        } else if is_remote {
            "○"
        } else {
            "◯"
        };
        let marker_colored = if head_marker == "*" {
            ui::success_bold(marker)
        } else if is_remote {
            ui::dimmed(marker)
        } else {
            ui::muted(marker)
        };

        let branch_colored = if head_marker == "*" {
            ui::success_bold(&display_name)
        } else if is_remote {
            ui::danger(&display_name)
        } else {
            ui::paint_text(&display_name)
        };

        // Truncate long subject lines to keep the table readable.
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
            ui::color_author(&if author.len() > 18 {
                format!("{}…", &author[..17])
            } else {
                author.to_string()
            }),
            ui::color_date(date),
            if upstream.is_empty() {
                ui::muted("—")
            } else {
                ui::color_branch(upstream)
            },
        ]);
    }

    table.print();
    ui::print_blank();
    Ok(())
}

// ─── Enhanced show ────────────────────────────────────────────────────────────

/// Show a commit's metadata with rich formatting, followed by its diff.
///
/// Displays the full hash, author, date (absolute + relative), subject, and
/// optional body in a readable layout, then delegates to [`enhanced_diff`] for
/// the patch view.
///
/// # Errors
///
/// Returns an error if any git operation fails or the config cannot be loaded.
pub fn enhanced_show(extra_args: &[String]) -> Result<()> {
    let rev = extra_args.first().map(|s| s.as_str()).unwrap_or("HEAD");

    let meta_fmt = "%H\x01%h\x01%s\x01%b\x01%an\x01%ae\x01%ai\x01%ar\x01%D\x01%P";
    let meta_raw = git_output_lossy(&["show", "-s", &format!("--format={}", meta_fmt), rev]);

    for line in meta_raw.lines() {
        let fields: Vec<&str> = line.splitn(10, '\x01').collect();
        if fields.len() >= 9 {
            let (_hash, _short_hash, subject, body, author, email, date_iso, date_rel, refs) = (
                fields[0], fields[1], fields[2], fields[3], fields[4], fields[5], fields[6],
                fields[7], fields[8],
            );

            // Fieldset header: "/////  abc1234  //  feat: add …  ////…"
            let short = _short_hash;
            let subject_preview: String = subject.chars().take(45).collect();
            ui::print_blank();
            ui::print_fieldset(&format!("{}  {}", short, subject_preview));
            ui::print_blank();
            ui::print_key_value_pairs(&[
                ("Author", ui::primary(&format!("{} <{}>", author, email))),
                ("Date", ui::muted(&format!("{}  ({})", date_iso, date_rel))),
                (
                    "Refs",
                    if refs.trim().is_empty() {
                        ui::muted("—")
                    } else {
                        ui::format_refs(refs)
                    },
                ),
            ]);
            if !body.trim().is_empty() {
                ui::print_blank();
                for body_line in body.lines() {
                    ui::print_line(&format!("      {}", ui::paint_text(body_line)));
                }
            }
            ui::print_blank();
            break;
        }
    }

    // Show the diff for this single commit.
    //
    // `<rev>^!` is git's shorthand for "<rev>^..<rev>" — "just the changes
    // introduced by this commit".  It works for all commits with a parent.
    // For the initial commit (no parent), we fall back to `--root <rev>` which
    // treats every file as added from nothing.
    let parents_field = meta_raw
        .lines()
        .next()
        .and_then(|l| l.splitn(10, '\x01').nth(9))
        .unwrap_or("")
        .trim();

    let diff_args: Vec<String> = if parents_field.is_empty() {
        // Initial commit — no parent exists; show everything as additions.
        let mut a = vec!["--root".to_string(), rev.to_string()];
        a.extend(extra_args.iter().filter(|&s| s != rev).cloned());
        a
    } else {
        // Normal commit — diff against first parent using the `^!` notation.
        let mut a = vec![format!("{}^!", rev)];
        a.extend(extra_args.iter().filter(|&s| s != rev).cloned());
        a
    };
    enhanced_diff(&diff_args)
}
