//! Interactive commit builder and commit execution.
//!
//! ## Tutorial overview
//!
//! This module implements the `g commit` command.  It provides two paths:
//!
//! - **Guided / interactive** — when no `--message` flag is given, the user is
//!   walked through a series of `dialoguer` prompts (type, scope, subject, body,
//!   footer) to build a [Conventional Commit] message.
//! - **Non-interactive** — when `--message` is given the prompts are skipped
//!   and the commit is made immediately.
//!
//! In both cases the commit is executed via [`gitcmd::git_output`] and the
//! result is displayed with a coloured success/error summary.
//!
//! [Conventional Commit]: https://www.conventionalcommits.org/

use anyhow::{bail, Result};
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use rusqlite::Connection;

use crate::cli::CommitArgs;
use crate::commands::git as gitcmd;
use crate::config;
use crate::storage::{repos, stats};
use crate::ui;

/// Entry point for `g commit`.
///
/// Determines the commit message (interactively or from flags), validates the
/// subject length, then invokes `git commit`.
///
/// # Errors
///
/// Returns an error if:
/// - The current directory is not a git repository.
/// - Staging with `-a` fails.
/// - Any interactive prompt fails (e.g. EOF on stdin).
/// - The commit message is rejected or the user cancels.
pub fn commit(conn: &Connection, args: &CommitArgs) -> Result<()> {
    let cfg = config::load()?;

    if !gitcmd::is_inside_git_repo() {
        bail!("Not inside a git repository.");
    }

    if gitcmd::is_dry_run() {
        return commit_dry_run(args, &cfg);
    }

    // Stage everything if -a was given.
    if args.all {
        gitcmd::git_output(&["add", "-A"])?;
    }

    // Bail out early if there is nothing staged (and we are not amending).
    let staged = gitcmd::git_output_lossy(&["diff", "--cached", "--name-only"]);
    if staged.is_empty() && !args.amend {
        ui::print_warning("Nothing staged to commit.");
        ui::print_tip(&format!(
            "Use {} or {} to stage changes.",
            "git add <file>".yellow(),
            format!("{} commit -a", crate::bin_name()).yellow()
        ));
        return Ok(());
    }

    // Show what's staged.
    show_staged_summary()?;

    // Build the commit message — either from flags or interactively.
    let message = match message_from_flags(args) {
        Some(m) => m,
        None => build_commit_message(args, &cfg)?,
    };

    // Warn if the subject line exceeds the configured maximum length.
    let subject_line = message.lines().next().unwrap_or(&message);
    if subject_line.len() > cfg.commit.max_subject_length {
        ui::print_warning(&format!(
            "Subject line is {} chars (max {})",
            subject_line.len(),
            cfg.commit.max_subject_length
        ));
        let proceed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Commit anyway?")
            .default(false)
            .interact()?;
        if !proceed {
            return Ok(());
        }
    }

    // Build the `git commit` argument list.
    let mut git_args = vec!["commit", "-m", &message];

    if args.no_verify {
        git_args.push("--no-verify");
    }
    if args.amend {
        git_args.push("--amend");
    }
    if cfg.commit.gpg_sign {
        git_args.push("-S");
    }

    ui::print_blank();
    let pb = ui::spinner("Committing…");
    let result = gitcmd::git_output(&git_args);

    match result {
        Ok(out) => {
            let hash = gitcmd::git_output_lossy(&["rev-parse", "--short", "HEAD"]);
            ui::spinner_success(
                pb,
                &format!(
                    "{} {}",
                    hash.yellow().bold(),
                    ui::color_subject(subject_line)
                ),
            );
            if !out.is_empty() {
                println!("  {}", out.lines().last().unwrap_or("").bright_black());
            }
            ui::print_blank();

            // Record stats — best-effort, never fails the commit.
            let (commit_type, scope) = parse_conventional_type(subject_line);
            let has_body = message.lines().count() > 2;
            let repo_id = gitcmd::repo_root()
                .ok()
                .and_then(|r| repos::find_id(conn, &r).ok().flatten());
            if let Some(rid) = repo_id {
                stats::record_commit(
                    conn,
                    rid,
                    commit_type.as_deref(),
                    scope.as_deref(),
                    has_body,
                    cfg.commit.gpg_sign,
                )
                .ok();
            }
        }
        Err(e) => {
            ui::spinner_error(pb, &format!("Commit failed: {}", e));
        }
    }

    Ok(())
}

/// Parse an optional conventional commit type and scope from a subject line.
///
/// `"feat(auth): add login"` → `(Some("feat"), Some("auth"))`
/// `"fix: typo"`             → `(Some("fix"), None)`
/// `"random message"`        → `(None, None)`
fn parse_conventional_type(subject: &str) -> (Option<String>, Option<String>) {
    // Match `type(scope): ...` or `type: ...`
    let before_colon = match subject.split_once(':') {
        Some((lhs, _)) => lhs,
        None => return (None, None),
    };

    if let Some((t, rest)) = before_colon.split_once('(') {
        let t = t.trim();
        let scope = rest.trim_end_matches(')').trim();
        if !t.is_empty() && t.chars().all(|c| c.is_alphanumeric() || c == '-') {
            return (Some(t.to_string()), Some(scope.to_string()));
        }
    }

    let t = before_colon.trim();
    if !t.is_empty() && t.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return (Some(t.to_string()), None);
    }

    (None, None)
}

/// Build a commit message from the `--message` / `--body` CLI flags, if set.
///
/// Returns `Some(message)` when `--message` is present, combining it with
/// `--body` separated by a blank line when a body is also given.
/// Returns `None` when `--message` is absent, signalling that the interactive
/// prompt flow should be used instead.
///
/// This function replaces two verbatim copies of the same `if let Some(msg)`
/// block that existed in [`commit`] and [`commit_dry_run`].
fn message_from_flags(args: &CommitArgs) -> Option<String> {
    let msg = args.message.as_ref()?;
    let body = args.body.as_deref().unwrap_or_default();
    if body.is_empty() {
        Some(msg.clone())
    } else {
        Some(format!("{}\n\n{}", msg, body))
    }
}

/// Show what the commit command would do in dry-run mode without executing it.
///
/// # Errors
///
/// Propagates any error from [`gitcmd::git_mutate`].
fn commit_dry_run(args: &CommitArgs, cfg: &config::Config) -> Result<()> {
    if args.all {
        gitcmd::git_mutate(&["add", "-A"], "Stage all tracked and untracked files")?;
    }

    let message_desc = message_from_flags(args)
        .unwrap_or_else(|| "<interactive prompt — message built via guided flow>".to_string());

    let mut git_args: Vec<&str> = vec!["commit", "-m"];
    // We must keep `msg_placeholder` alive for the lifetime of `git_args`.
    let msg_placeholder;
    if args.message.is_some() {
        msg_placeholder = message_desc.clone();
        git_args.push(&msg_placeholder);
    } else {
        git_args.push("<message>");
    }

    if args.no_verify {
        git_args.push("--no-verify");
    }
    if args.amend {
        git_args.push("--amend");
    }
    if cfg.commit.gpg_sign {
        git_args.push("-S");
    }

    let explanation = if args.amend {
        "Amend the previous commit with staged changes"
    } else {
        "Create a new commit with staged changes"
    };

    gitcmd::git_mutate(&git_args, explanation)?;

    if args.message.is_some() {
        println!(
            "           {} {}",
            "message:".bright_black(),
            message_desc.bright_black()
        );
    } else {
        println!(
            "           {} {}",
            "note:".bright_black(),
            "Commit message would be built via interactive prompts".bright_black()
        );
    }

    Ok(())
}

/// Print a short diffstat summary of the currently staged changes.
fn show_staged_summary() -> Result<()> {
    let stat = gitcmd::git_output_lossy(&["diff", "--cached", "--stat"]);
    if stat.is_empty() {
        return Ok(());
    }

    ui::print_section("Staged changes", None);
    // Show at most 12 file lines to keep the output concise.
    for line in stat.lines().take(12) {
        let colored = colorize_stat_line(line);
        println!("  {}", colored);
    }
    if stat.lines().count() > 12 {
        println!("  {}", "…and more".bright_black());
    }
    ui::print_blank();
    Ok(())
}

/// Colorise a single line from `git diff --stat`.
///
/// Turns `+` characters green and `-` characters red while keeping the rest white.
fn colorize_stat_line(line: &str) -> String {
    if line.contains('|') {
        let parts: Vec<&str> = line.splitn(2, '|').collect();
        let file = parts[0];
        let rest = parts[1];
        let rest_colored = rest
            .replace('+', &"+".green().to_string())
            .replace('-', &"-".red().to_string());
        format!("{}{}{}", file.white(), "|".bright_black(), rest_colored)
    } else {
        // Summary line, e.g. "3 files changed, 42 insertions(+), 7 deletions(-)"
        line.bright_black().to_string()
    }
}

/// Build a Conventional Commit message using interactive `dialoguer` prompts.
///
/// Steps:
/// 1. **Type** — select from the configured commit types.
/// 2. **Scope** — optional component/area tag (can be skipped).
/// 3. **Subject** — imperative, present-tense description (validated for length).
/// 4. **Body** — optional multi-line explanation of *why* the change was made.
/// 5. **Footer** — optional `BREAKING CHANGE` or `closes #N` annotation.
///
/// # Errors
///
/// Returns an error if any prompt interaction fails (e.g. the user sends EOF)
/// or if the user cancels at the final confirmation.
fn build_commit_message(args: &CommitArgs, cfg: &config::Config) -> Result<String> {
    let theme = ColorfulTheme::default();

    ui::print_info("Building commit message…");
    ui::print_blank();

    // Step 1: Choose commit type.
    let commit_type = if let Some(t) = &args.r#type {
        t.clone()
    } else {
        let type_descriptions: Vec<String> = cfg
            .commit
            .types
            .iter()
            .map(|t| format_type_label(t, cfg.commit.emoji))
            .collect();

        let idx = Select::with_theme(&theme)
            .with_prompt("  Commit type")
            .items(&type_descriptions)
            .default(0)
            .interact()?;

        cfg.commit.types[idx].clone()
    };

    // Step 2: Optional scope.
    let scope = if let Some(s) = &args.scope {
        if s.is_empty() {
            None
        } else {
            Some(s.clone())
        }
    } else if cfg.commit.require_scope {
        let s: String = Input::with_theme(&theme)
            .with_prompt("  Scope (component/area)")
            .interact_text()?;
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    } else {
        let s: String = Input::with_theme(&theme)
            .with_prompt("  Scope (optional, press Enter to skip)")
            .allow_empty(true)
            .interact_text()?;
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    };

    // Step 3: Subject line.
    let subject: String = Input::with_theme(&theme)
        .with_prompt("  Subject (imperative, present tense)")
        .validate_with(|input: &String| {
            if input.trim().is_empty() {
                Err("Subject cannot be empty")
            } else if input.len() > cfg.commit.max_subject_length {
                Err("Subject is too long")
            } else {
                Ok(())
            }
        })
        .interact_text()?;

    // Build the first line of the commit message.
    let first_line = if let Some(sc) = &scope {
        format!("{}({}): {}", commit_type, sc, subject.trim())
    } else {
        format!("{}: {}", commit_type, subject.trim())
    };

    // Show a preview before asking for the body.
    ui::print_blank();
    ui::print_key_value_pairs(&[("Preview", ui::color_subject(&first_line).bold().to_string())]);
    ui::print_blank();

    // Step 4: Optional body.
    let body = if cfg.commit.require_body {
        let b: String = Input::with_theme(&theme)
            .with_prompt("  Body (explain WHY, not WHAT)")
            .interact_text()?;
        b
    } else {
        let add_body = Confirm::with_theme(&theme)
            .with_prompt("  Add a body? (explain WHY, motivation, context)")
            .default(false)
            .interact()?;

        if add_body {
            println!(
                "  {} Enter body (empty line to finish, or single dot to skip):",
                "→".cyan()
            );
            let mut lines = vec![];
            loop {
                let line: String = Input::with_theme(&theme)
                    .with_prompt("  ")
                    .allow_empty(true)
                    .interact_text()?;
                if line == "." || (line.is_empty() && !lines.is_empty()) {
                    break;
                }
                if !line.is_empty() {
                    lines.push(line);
                }
            }
            lines.join("\n")
        } else {
            String::new()
        }
    };

    // Step 5: Optional footer (BREAKING CHANGE, closes #N, …).
    let add_footer = Confirm::with_theme(&theme)
        .with_prompt("  Add footer? (BREAKING CHANGE, closes #issue, etc.)")
        .default(false)
        .interact()?;

    let footer = if add_footer {
        let f: String = Input::with_theme(&theme)
            .with_prompt("  Footer")
            .allow_empty(true)
            .interact_text()?;
        f
    } else {
        String::new()
    };

    // Assemble the full message.
    let mut message = first_line.clone();
    if !body.is_empty() {
        message.push_str("\n\n");
        message.push_str(&body);
    }
    if !footer.is_empty() {
        message.push_str("\n\n");
        message.push_str(&footer);
    }

    // Final full preview before confirmation.
    ui::print_blank();
    ui::print_divider();
    for line in message.lines() {
        println!("  {}", line.white());
    }
    ui::print_divider();
    ui::print_blank();

    let confirm = Confirm::with_theme(&theme)
        .with_prompt("  Commit with this message?")
        .default(true)
        .interact()?;

    if !confirm {
        bail!("Commit cancelled.");
    }

    Ok(message)
}

/// Render a commit-type label for the interactive picker.
///
/// When `emoji` is `true` an icon is prepended to the type name.  A
/// description is appended in dim colour for all recognised types.
fn format_type_label(t: &str, emoji: bool) -> String {
    let (icon, description) = match t {
        "feat" => ("✨", "A new feature"),
        "fix" => ("🐛", "A bug fix"),
        "docs" => ("📝", "Documentation changes"),
        "style" => ("💅", "Formatting, style changes"),
        "refactor" => ("♻️ ", "Code refactoring"),
        "perf" => ("⚡", "Performance improvements"),
        "test" => ("✅", "Adding or fixing tests"),
        "build" => ("🏗️ ", "Build system changes"),
        "ci" => ("👷", "CI/CD changes"),
        "chore" => ("🔧", "Other changes (no src/test)"),
        "revert" => ("⏪", "Reverting a previous commit"),
        _ => ("·", ""),
    };
    if description.is_empty() {
        if emoji {
            format!("{} {}", icon, t)
        } else {
            t.to_string()
        }
    } else {
        // Both emoji=true and emoji=false produce the same output for known types,
        // because the description column already carries the meaning.
        format!("{} {:12}  {}", icon, t, description.bright_black())
    }
}
