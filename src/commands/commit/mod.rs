//! Interactive commit builder and commit execution.
//!
//! `g commit` has two paths:
//!
//! - **Guided / interactive** — when no `--message` flag is given, the user is
//!   walked through a series of prompts (type, scope, subject, body, footer)
//!   to build a [Conventional Commit] message.
//! - **Non-interactive** — when `--message` is given the prompts are skipped
//!   and the commit is made immediately.
//!
//! In both cases the commit is executed via [`gitcmd::git_output`] and the
//! result is displayed with a coloured success/error summary.
//!
//! [Conventional Commit]: https://www.conventionalcommits.org/
//!
//! # Folder layout
//!
//! ```text
//! commit/
//!   mod.rs      ← this file: public `commit()` entry point + orchestration
//!   parse.rs    ← pure helpers (extract_body, parse_conventional_type,
//!                 message_from_flags) with unit tests
//!   preview.rs  ← display helpers (show_staged_summary, colorize_stat_line,
//!                 type_label_parts)
//!   dry_run.rs  ← `--dry-run` path: print planned `git commit` invocation
//!   builder.rs  ← interactive + inline commit-message builders + type picker
//! ```

mod builder;
mod dry_run;
mod parse;
mod preview;

use std::io::IsTerminal;

use crate::cli::CommitArgs;
use crate::commands::git as gitcmd;
use crate::commands::{Ctx, Error as CommandError};
use crate::config;
use crate::storage::{repos, stats};
use crate::ui;
use anyhow::{bail, Result};

use builder::{build_commit_message_inline, build_commit_message_interactive};
use dry_run::commit_dry_run;
use parse::{extract_body, message_from_flags, parse_conventional_type};
use preview::show_staged_summary;

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
pub fn commit(ctx: &Ctx, args: &CommitArgs) -> Result<()> {
    let conn = ctx.conn;
    let cfg = config::load()?;

    if !gitcmd::is_inside_git_repo() {
        return Err(CommandError::NotInRepo.into());
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
            // the builder dispatches to the non-fullscreen inline variants
            // automatically.
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

                // Record full commit message for statistics and search
                let full_hash = gitcmd::git_output_lossy(&["rev-parse", "HEAD"]);
                let body = extract_body(&message);
                let author_info = gitcmd::git_output_lossy(&["log", "-1", "--format=%an%x00%ae"]);
                let (author_name, author_email) = author_info
                    .split_once('\x00')
                    .map(|(n, e)| (Some(n.to_string()), Some(e.to_string())))
                    .unwrap_or((None, None));
                let committed_at = chrono::Utc::now().to_rfc3339();

                stats::record_commit_message(
                    conn,
                    rid,
                    &full_hash,
                    subject_line,
                    body.as_deref(),
                    author_name.as_deref(),
                    author_email.as_deref(),
                    &committed_at,
                    false, // not imported
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
