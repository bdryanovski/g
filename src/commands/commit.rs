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

use std::io::IsTerminal;

use anyhow::{bail, Result};
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
            ui::warning("git add <file>"),
            ui::warning(&format!("{} commit -a", crate::bin_name()))
        ));
        return Ok(());
    }

    // Show what's staged.
    show_staged_summary()?;

    // Build the commit message — either from flags or interactively.
    let message = match message_from_flags(args) {
        Some(m) => m,
        None => {
            if !std::io::stdin().is_terminal() {
                bail!(
                    "Interactive commit requires a TTY.\n\
                     Use `{} commit --message \"your message\"` for non-interactive use.",
                    crate::bin_name()
                );
            }
            // Route to the appropriate interactive builder.
            //
            // "interactive" (default) — full-screen ratatui TUI, alternate screen.
            // "inline" / prompt_mode  — sequential prompts that stay in scrollback.
            //
            // When commit_mode = "inline" the global INLINE_PROMPTS flag is set
            // so that every ui::select / ui::input / ui::confirm call within
            // the builder (including select_commit_type) dispatches to the
            // non-fullscreen inline variants automatically.
            if cfg.ui.commit_mode == "inline" {
                ui::set_inline_prompts();
            }
            if ui::is_inline_prompts() {
                build_commit_message_inline(args, &cfg)?
            } else {
                build_commit_message_interactive(args, &cfg)?
            }
        }
    };

    // Warn if the subject line exceeds the configured maximum length.
    let subject_line = message.lines().next().unwrap_or(&message);
    if subject_line.len() > cfg.commit.max_subject_length {
        ui::print_warning(&format!(
            "Subject line is {} chars (max {})",
            subject_line.len(),
            cfg.commit.max_subject_length
        ));
        if !ui::confirm("Subject is long — commit anyway?", false) {
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
    // Append "Signed-off-by: Name <email>" trailer — set via [commit] sign_off = true.
    if cfg.commit.sign_off {
        git_args.push("--signoff");
    }
    // GPG-sign the commit object — set via [commit] gpg_sign = true.
    if cfg.commit.gpg_sign {
        git_args.push("--gpg-sign");
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
                    ui::warning_bold(&hash),
                    ui::color_subject(subject_line)
                ),
            );
            if !out.is_empty() {
                ui::print_indented(&ui::muted(out.lines().last().unwrap_or("")));
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
    if cfg.commit.sign_off {
        git_args.push("--signoff");
    }
    if cfg.commit.gpg_sign {
        git_args.push("--gpg-sign");
    }

    let explanation = if args.amend {
        "Amend the previous commit with staged changes"
    } else {
        "Create a new commit with staged changes"
    };

    gitcmd::git_mutate(&git_args, explanation)?;

    if args.message.is_some() {
        ui::print_line(&format!(
            "           {} {}",
            ui::muted("message:"),
            ui::muted(&message_desc)
        ));
    } else {
        ui::print_line(&format!(
            "           {} {}",
            ui::muted("note:"),
            ui::muted("Commit message would be built via interactive prompts")
        ));
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
        ui::print_indented(&colorize_stat_line(line));
    }
    if stat.lines().count() > 12 {
        ui::print_indented(&ui::muted("…and more"));
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
            .replace('+', &ui::success("+"))
            .replace('-', &ui::danger("-"));
        format!("{}{}{}", ui::paint_text(file), ui::muted("|"), rest_colored)
    } else {
        // Summary line, e.g. "3 files changed, 42 insertions(+), 7 deletions(-)"
        ui::muted(line)
    }
}

/// Show the commit-type picker and return the chosen type string.
///
/// Appends an **Other…** option at the end of the configured type list so the
/// user can enter any arbitrary type without editing `config.toml`.  When
/// "Other…" is chosen, a follow-up text prompt is shown.
///
/// Uses `ui::select` and `ui::input`, which both respect the global
/// `INLINE_PROMPTS` flag — so this function works identically in both the
/// full-screen and inline commit builders.
///
/// # Errors
///
/// Returns `Err` when the user cancels.
fn select_commit_type(args: &CommitArgs, cfg: &config::Config) -> Result<String> {
    if let Some(t) = &args.r#type {
        return Ok(t.clone());
    }

    // Build the option list from config types + a trailing "other" entry.
    let mut options: Vec<ui::SelectOption> = cfg
        .commit
        .types
        .iter()
        .map(|t| {
            let (_, description) = type_label_parts(t);
            if description.is_empty() {
                ui::SelectOption::new(t.clone())
            } else {
                ui::SelectOption::with_description(t.clone(), description)
            }
        })
        .collect();
    options.push(ui::SelectOption::with_description(
        "other".to_string(),
        "Custom type — enter manually",
    ));

    let idx = ui::select("Commit Builder — Type", &options)
        .ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?;

    // Last item = "other" → prompt for a free-form type.
    if idx == options.len() - 1 {
        let custom = ui::input("Commit Builder — Custom type", None)
            .ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?;
        let custom = custom.trim().to_string();
        if custom.is_empty() {
            anyhow::bail!("Commit cancelled — custom type cannot be empty.");
        }
        Ok(custom)
    } else {
        Ok(cfg.commit.types[idx].clone())
    }
}

/// Build a Conventional Commit message using the full-screen ratatui TUI.
///
/// Used when `[ui] commit_mode = "interactive"` (default).  Each step opens
/// a dedicated TUI screen with a ratatui-cheese Help bar at the bottom.
///
/// # Errors
///
/// Returns `Err` if the user cancels at any step.
fn build_commit_message_interactive(args: &CommitArgs, cfg: &config::Config) -> Result<String> {
    // Step 1: Type selection (shared helper handles "Other…" too).
    let commit_type = select_commit_type(args, cfg)?;

    // Step 2: Scope.
    let scope = if let Some(s) = &args.scope {
        if s.is_empty() {
            None
        } else {
            Some(s.clone())
        }
    } else {
        let prompt = if cfg.commit.require_scope {
            "Commit Builder — Scope (required)"
        } else {
            "Commit Builder — Scope (optional, Enter to skip)"
        };
        let s = ui::input(prompt, None).ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?;
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    };

    // Step 3: Subject.
    let max_len = cfg.commit.max_subject_length;
    let subject = ui::input_validated("Commit Builder — Subject", None, move |val| {
        if val.trim().is_empty() {
            Err("Subject cannot be empty".to_string())
        } else if val.len() > max_len {
            Err(format!("Subject is too long ({}/{})", val.len(), max_len))
        } else {
            Ok(())
        }
    })
    .ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?;

    let first_line = if let Some(sc) = &scope {
        format!("{}({}): {}", commit_type, sc, subject.trim())
    } else {
        format!("{}: {}", commit_type, subject.trim())
    };

    // Step 4: Body.
    let body = if cfg.commit.require_body {
        ui::input("Commit Builder — Body (explain WHY, not WHAT)", None)
            .ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?
    } else if ui::confirm("Commit Builder — Add a body?", false) {
        ui::input("Commit Builder — Body", None)
            .ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?
    } else {
        String::new()
    };

    // Step 5: Footer.
    let footer = if ui::confirm(
        "Commit Builder — Add footer? (BREAKING CHANGE, closes #N…)",
        false,
    ) {
        ui::input("Commit Builder — Footer", None)
            .ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?
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

    // Final preview + confirmation.
    ui::print_blank();
    ui::print_fieldset("Commit Builder — Preview");
    ui::print_blank();
    for line in message.lines() {
        ui::print_indented(&ui::paint_text(line));
    }
    ui::print_blank();

    if !ui::confirm("Commit with this message?", true) {
        bail!("Commit cancelled.");
    }

    Ok(message)
}

/// Build a Conventional Commit message using inline (non-fullscreen) prompts.
///
/// Used when `[ui] commit_mode = "inline"`.  Each step prints its prompt and
/// the user's answer into the normal terminal scroll buffer — the commit
/// history is visible after the command completes and no alternate screen is
/// entered or restored.
///
/// The steps mirror [`build_commit_message_interactive`] exactly; only the
/// prompt mechanism changes.
///
/// # Errors
///
/// Returns `Err` if the user cancels at any step.
fn build_commit_message_inline(args: &CommitArgs, cfg: &config::Config) -> anyhow::Result<String> {
    ui::print_blank();
    ui::print_fieldset("Commit Builder");

    // ── Step 1: Type (shared helper handles "Other…") ────────────────────────
    let commit_type = select_commit_type(args, cfg)?;

    // ── Step 2: Scope ─────────────────────────────────────────────────────────
    let scope = if let Some(s) = &args.scope {
        if s.is_empty() {
            None
        } else {
            Some(s.clone())
        }
    } else {
        let prompt = if cfg.commit.require_scope {
            "Commit Builder — Scope (required)"
        } else {
            "Commit Builder — Scope (optional, Enter to skip)"
        };
        let s = ui::input(prompt, None).ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?;
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    };

    // ── Step 3: Subject ───────────────────────────────────────────────────────
    let max_len = cfg.commit.max_subject_length;
    let subject = ui::input_validated("Commit Builder — Subject", None, move |val| {
        if val.trim().is_empty() {
            Err("Subject cannot be empty".to_string())
        } else if val.len() > max_len {
            Err(format!("Subject is too long ({}/{})", val.len(), max_len))
        } else {
            Ok(())
        }
    })
    .ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?;

    let first_line = match &scope {
        Some(sc) => format!("{}({}): {}", commit_type, sc, subject.trim()),
        None => format!("{}: {}", commit_type, subject.trim()),
    };

    // ── Step 4: Body ──────────────────────────────────────────────────────────
    let body = if cfg.commit.require_body {
        ui::input("Commit Builder — Body (explain WHY, not WHAT)", None)
            .ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?
    } else if ui::confirm("Commit Builder — Add a body?", false) {
        ui::input("Commit Builder — Body", None)
            .ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?
    } else {
        String::new()
    };

    // ── Step 5: Footer ────────────────────────────────────────────────────────
    let footer = if ui::confirm(
        "Commit Builder — Add footer? (BREAKING CHANGE, closes #N…)",
        false,
    ) {
        ui::input("Commit Builder — Footer", None)
            .ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?
    } else {
        String::new()
    };

    // ── Assemble message ──────────────────────────────────────────────────────
    let mut message = first_line.clone();
    if !body.is_empty() {
        message.push_str("\n\n");
        message.push_str(&body);
    }
    if !footer.is_empty() {
        message.push_str("\n\n");
        message.push_str(&footer);
    }

    // ── Preview + confirmation ────────────────────────────────────────────────
    ui::print_blank();
    ui::print_fieldset("Commit Builder — Preview");
    ui::print_blank();
    for line in message.lines() {
        ui::print_indented(&ui::paint_text(line));
    }
    ui::print_blank();

    if !ui::confirm("Commit Builder — Commit with this message?", true) {
        bail!("Commit cancelled.");
    }

    Ok(message)
}

/// Return the `(emoji_icon, description)` for a conventional commit type label.
fn type_label_parts(t: &str) -> (&'static str, &'static str) {
    match t {
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
    }
}
