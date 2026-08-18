//! `g branch` — colourised list, plus `branch squash` for single-branch squashes.
//!
//! [`dispatch_branch`] routes to either [`branch_squash`] (when the user passed
//! the `squash` subcommand) or [`enhanced_branch`] (the colourised table /
//! `git branch` passthrough for mutating flags).
//!
//! The branch list supports interactive fuzzy search — press `/` to filter branches.

use anyhow::{bail, Context, Result};
use std::process::{Command, Stdio};

use crate::commands::Error as CommandError;
use crate::ui;

use super::dry_run::{git_mutate, is_dry_run};
use super::exec::{git_exe, git_output, git_output_lossy, passthrough, require_clean_tree};
use super::repo::{current_branch, default_branch};

/// Branch info for display and selection.
#[derive(Clone)]
struct BranchInfo {
    name: String,
    hash: String,
    subject: String,
    author: String,
    date: String,
    upstream: String,
    is_head: bool,
    is_remote_only: bool,
}

// ─── Dispatch ────────────────────────────────────────────────────────────────

/// Route `g branch [...]`: if the user passed the `squash` subcommand, run
/// [`branch_squash`]; otherwise hand off to [`enhanced_branch`] (which may
/// fall through to a `git branch` passthrough).
pub fn dispatch_branch(args: crate::cli::BranchArgs) -> Result<()> {
    if let Some(crate::cli::BranchSquashCmd::Squash { message, base }) = args.cmd {
        branch_squash(message.as_deref(), base.as_deref())
    } else {
        enhanced_branch(&args.rest)
    }
}

// ─── Squash helpers ──────────────────────────────────────────────────────────

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
/// Used both by [`branch_squash`] here and by the stack-squash subcommand,
/// so it lives in the public surface of the git module.
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

// ─── branch squash ───────────────────────────────────────────────────────────

/// Collapse all commits on the current branch into a single commit on top of
/// its merge-base with `base`.
///
/// Steps:
/// 1. Compute `git merge-base HEAD <base>`.
/// 2. `git reset --soft <merge-base>` to stage all branch changes at once.
/// 3. `git commit -m <message>` to create the single squashed commit.
pub fn branch_squash(message: Option<&str>, base: Option<&str>) -> Result<()> {
    require_clean_tree("squashing")?;
    let branch = current_branch()?;
    if branch == "HEAD" {
        return Err(CommandError::DetachedHead.into());
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

// ─── enhanced branch ─────────────────────────────────────────────────────────

/// List branches with metadata and colour, or pass through for mutation flags.
///
/// When `extra_args` contains flags that create, delete, or move branches
/// (`-d`, `-D`, `-m`, `--move`, `--copy`, `-b`, `--create`), the call is
/// forwarded to `git branch` unchanged.
///
/// Otherwise a formatted table is printed showing branch name, hash, last
/// commit subject, author, date, and upstream tracking branch.
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

    if mutating
        || (!extra_args.is_empty()
            && !extra_args[0].starts_with('-')
            && extra_args[0] != "-a"
            && extra_args[0] != "--all")
    {
        let mut args = vec!["branch".to_string()];
        args.extend_from_slice(extra_args);
        return passthrough(&args);
    }

    // Check if -a/--all flag is present - if so, fetch from remote first
    let show_all = extra_args.iter().any(|a| a == "-a" || a == "--all");

    if show_all {
        // Fetch latest refs from remote (silently)
        let _ = std::process::Command::new(git_exe())
            .args(["fetch", "--prune"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }

    // Get local branches with their tracking info
    let local_raw = git_output_lossy(&[
        "branch",
        "--format=%(refname:short)\t%(objectname:short)\t%(subject)\t%(authorname)\t%(committerdate:relative)\t%(upstream:short)\t%(HEAD)",
    ]);

    // Get list of branches merged into HEAD to show merge status
    let merged_output = git_output_lossy(&["branch", "--merged", "HEAD"]);
    let merged_branches: std::collections::HashSet<String> = merged_output
        .lines()
        .map(|l| l.trim().trim_start_matches("* ").to_string())
        .collect();

    // Build a set of remote branches that are tracked by local branches
    let mut tracked_remotes: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Collect local branches info
    let mut branches: Vec<BranchInfo> = Vec::new();

    for line in local_raw.lines() {
        let fields: Vec<&str> = line.splitn(7, '\t').collect();
        if fields.len() < 7 {
            continue;
        }
        let (name, hash, subject, author, date, upstream, head_marker) = (
            fields[0], fields[1], fields[2], fields[3], fields[4], fields[5], fields[6],
        );

        if !upstream.is_empty() {
            tracked_remotes.insert(upstream.to_string());
        }

        branches.push(BranchInfo {
            name: name.to_string(),
            hash: hash.to_string(),
            subject: subject.to_string(),
            author: author.to_string(),
            date: date.to_string(),
            upstream: upstream.to_string(),
            is_head: head_marker == "*",
            is_remote_only: false,
        });
    }

    // If -a flag, also get remote-only branches (those not tracked by any local branch)
    if show_all {
        let remote_raw = git_output_lossy(&[
            "branch",
            "-r",
            "--format=%(refname:short)\t%(objectname:short)\t%(subject)\t%(authorname)\t%(committerdate:relative)",
        ]);

        for line in remote_raw.lines() {
            let fields: Vec<&str> = line.splitn(5, '\t').collect();
            if fields.len() < 5 {
                continue;
            }
            let (name, hash, subject, author, date) =
                (fields[0], fields[1], fields[2], fields[3], fields[4]);

            // Skip HEAD pointer (e.g., "origin/HEAD") and bare remote name (e.g., "origin")
            if name.contains("/HEAD") || !name.contains('/') {
                continue;
            }

            // Skip if this remote is already tracked by a local branch
            if tracked_remotes.contains(name) {
                continue;
            }

            // Skip if there's a local branch with the same name (after remote prefix)
            let short_name = name.split('/').skip(1).collect::<Vec<_>>().join("/");
            let has_local = branches.iter().any(|b| b.name == short_name);
            if has_local {
                continue;
            }

            branches.push(BranchInfo {
                name: name.to_string(),
                hash: hash.to_string(),
                subject: subject.to_string(),
                author: author.to_string(),
                date: date.to_string(),
                upstream: "—".to_string(),
                is_head: false,
                is_remote_only: true,
            });
        }
    }

    ui::print_blank();
    // Marker (0), Branch (1), Merged (2) and Hash (3) are protected — never
    // truncated. Metadata columns are flexible: on narrow terminals they shrink
    // (Last Commit first, as the widest) so the branch name always fits.
    let mut table = ui::Table::new(vec![
        "",
        "Branch",
        "Merged",
        "Hash",
        "Last Commit",
        "Author",
        "Date",
        "Tracking",
    ])
    .set_flexible(4, 8)
    .set_flexible(5, 6)
    .set_flexible(6, 7)
    .set_flexible(7, 6);

    for branch in &branches {
        let marker = if branch.is_head {
            "◉"
        } else if branch.is_remote_only {
            "○"
        } else {
            "◯"
        };
        let marker_colored = if branch.is_head {
            ui::success_bold(marker)
        } else {
            ui::muted(marker)
        };

        let branch_colored = if branch.is_head {
            ui::success_bold(&branch.name)
        } else if branch.is_remote_only {
            ui::muted(&branch.name)
        } else {
            ui::paint_text(&branch.name)
        };

        // Check if branch is merged into current HEAD
        let is_merged = merged_branches.contains(&branch.name) || branch.is_head;
        let merged_indicator = if is_merged {
            ui::success_bold("✓")
        } else {
            ui::muted("—")
        };

        // Subject and author are passed in full: the table grows them to fit a
        // wide terminal and only truncates (with `…`) when space is tight.
        table.add_row(vec![
            marker_colored,
            branch_colored,
            merged_indicator,
            ui::color_hash(&branch.hash),
            ui::color_subject(&branch.subject),
            ui::color_author(&branch.author),
            ui::color_date(&branch.date),
            if branch.upstream.is_empty() || branch.upstream == "—" {
                ui::muted("—")
            } else {
                ui::color_branch(&branch.upstream)
            },
        ]);
    }

    table.print();
    ui::print_blank();

    // Interactive mode: wait for keypress to enable fuzzy search
    if ui::is_no_interactive() {
        return Ok(());
    }

    // Show hint for interactive search
    println!(
        "  {} {} to search, {} to quit",
        ui::muted("Press"),
        ui::accent("/"),
        ui::muted("q")
    );
    ui::print_blank();

    // Wait for keypress
    if let Some(selected_branch) = wait_for_branch_selection(&branches)? {
        // Checkout the selected branch
        if selected_branch != current_branch().unwrap_or_default() {
            ui::print_info(&format!("Switching to '{}'...", selected_branch));
            git_output(&["checkout", &selected_branch])?;
            ui::print_success(&format!("Switched to '{}'", selected_branch));
        }
    }

    Ok(())
}

/// Wait for user input: `/` to search, `q` to quit, or timeout.
fn wait_for_branch_selection(branches: &[BranchInfo]) -> Result<Option<String>> {
    use crossterm::event::{self, Event, KeyCode, KeyEvent};
    use crossterm::terminal;
    use std::time::Duration;

    // Enable raw mode to capture single keypresses
    terminal::enable_raw_mode().context("Failed to enable raw mode")?;

    let result = loop {
        // Poll for events with a timeout
        if event::poll(Duration::from_secs(30)).unwrap_or(false) {
            if let Ok(Event::Key(KeyEvent { code, .. })) = event::read() {
                match code {
                    KeyCode::Char('/') => {
                        terminal::disable_raw_mode().ok();
                        // Enter fuzzy search mode with table-style display
                        if let Some(branch) = fuzzy_branch_picker(branches) {
                            break Ok(Some(branch));
                        }
                        break Ok(None);
                    }
                    KeyCode::Char('q') | KeyCode::Esc => {
                        break Ok(None);
                    }
                    _ => {}
                }
            }
        } else {
            // Timeout - exit quietly
            break Ok(None);
        }
    };

    terminal::disable_raw_mode().ok();
    result
}

/// Fuzzy branch picker with table-style display.
fn fuzzy_branch_picker(branches: &[BranchInfo]) -> Option<String> {
    use crossterm::event::{self, Event, KeyCode, KeyModifiers};
    use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
    use crossterm::{cursor, execute};
    use std::io::{stdout, Write};

    // State
    let mut query = String::new();
    let mut cursor_pos: usize = 0;
    let mut scroll_offset: usize = 0;

    // Enter alternate screen
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide).ok();
    terminal::enable_raw_mode().ok();

    let result = loop {
        // Filter branches by query
        let filtered: Vec<&BranchInfo> = if query.is_empty() {
            branches.iter().collect()
        } else {
            let q = query.to_lowercase();
            branches
                .iter()
                .filter(|b| b.name.to_lowercase().contains(&q))
                .collect()
        };

        // Get terminal size
        let (term_width, term_height) = terminal::size().unwrap_or((80, 24));
        let visible_rows = (term_height as usize).saturating_sub(5); // Header + search + footer

        // Clamp cursor to valid range
        if filtered.is_empty() {
            cursor_pos = 0;
        } else {
            cursor_pos = cursor_pos.min(filtered.len() - 1);
        }

        // Adjust scroll offset to keep cursor visible
        if cursor_pos < scroll_offset {
            scroll_offset = cursor_pos;
        } else if cursor_pos >= scroll_offset + visible_rows {
            scroll_offset = cursor_pos - visible_rows + 1;
        }

        // Clear screen and render
        execute!(
            stdout,
            cursor::MoveTo(0, 0),
            terminal::Clear(terminal::ClearType::All)
        )
        .ok();

        // Header
        println!("  {}", ui::primary_bold("Switch to branch"));
        println!();

        // Search box
        println!("  {} {}{}", ui::muted("Search:"), query, ui::muted("▌"));
        println!();

        // Table header
        let name_width = 35.min(term_width as usize / 3);
        println!(
            "  {}  {:<width$}  {}  {}",
            ui::muted(" "),
            ui::muted("Branch"),
            ui::muted("Hash"),
            ui::muted("Last Commit"),
            width = name_width
        );
        println!(
            "  {}",
            ui::muted(&"─".repeat((term_width as usize).saturating_sub(4)))
        );

        // Render visible branches
        let visible_branches: Vec<_> = filtered
            .iter()
            .skip(scroll_offset)
            .take(visible_rows)
            .collect();

        for (i, branch) in visible_branches.iter().enumerate() {
            let actual_idx = scroll_offset + i;
            let is_selected = actual_idx == cursor_pos;

            let marker = if branch.is_head {
                "◉"
            } else if branch.is_remote_only {
                "○"
            } else {
                "◯"
            };

            // Truncate branch name
            let name_display = if branch.name.len() > name_width {
                format!("{}…", &branch.name[..name_width - 1])
            } else {
                format!("{:<width$}", branch.name, width = name_width)
            };

            // Truncate subject
            let subject_width = (term_width as usize).saturating_sub(name_width + 20);
            let subject_display = if branch.subject.len() > subject_width {
                format!("{}…", &branch.subject[..subject_width.saturating_sub(1)])
            } else {
                branch.subject.clone()
            };

            if is_selected {
                // Highlighted row with dim background
                print!("\x1b[48;5;236m"); // Dim gray background
                println!(
                    "  {}  {}  {}  {}\x1b[0m",
                    ui::success_bold(marker),
                    ui::primary_bold(&name_display),
                    ui::color_hash(&branch.hash),
                    ui::color_subject(&subject_display),
                );
            } else {
                let marker_colored = if branch.is_head {
                    ui::success_bold(marker)
                } else {
                    ui::muted(marker)
                };

                let name_colored = if branch.is_head {
                    ui::success_bold(&name_display)
                } else if branch.is_remote_only {
                    ui::muted(&name_display)
                } else {
                    ui::paint_text(&name_display)
                };

                println!(
                    "  {}  {}  {}  {}",
                    marker_colored,
                    name_colored,
                    ui::color_hash(&branch.hash),
                    ui::muted(&subject_display),
                );
            }
        }

        // Show scroll indicators if needed
        if filtered.len() > visible_rows {
            println!();
            println!(
                "  {} {}/{}",
                ui::muted("Showing"),
                cursor_pos + 1,
                filtered.len()
            );
        }

        // Footer
        println!();
        println!(
            "  {}  {}  {}  {}",
            ui::muted("↑/↓ move"),
            ui::muted("Enter select"),
            ui::muted("Esc cancel"),
            ui::muted("Type to filter")
        );

        stdout.flush().ok();

        // Handle input
        if let Ok(Event::Key(key)) = event::read() {
            match key.code {
                KeyCode::Esc => break None,
                KeyCode::Enter => {
                    if !filtered.is_empty() {
                        break Some(filtered[cursor_pos].name.clone());
                    }
                }
                KeyCode::Up | KeyCode::Char('k')
                    if key.modifiers.is_empty() || key.code == KeyCode::Up =>
                {
                    cursor_pos = cursor_pos.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j')
                    if key.modifiers.is_empty() || key.code == KeyCode::Down =>
                {
                    if !filtered.is_empty() {
                        cursor_pos = (cursor_pos + 1).min(filtered.len() - 1);
                    }
                }
                KeyCode::Backspace => {
                    query.pop();
                    cursor_pos = 0;
                    scroll_offset = 0;
                }
                KeyCode::Char(c)
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                {
                    query.push(c);
                    cursor_pos = 0;
                    scroll_offset = 0;
                }
                _ => {}
            }
        }
    };

    // Leave alternate screen
    terminal::disable_raw_mode().ok();
    execute!(stdout, LeaveAlternateScreen, cursor::Show).ok();

    result
}
