//! `g stage` — interactive file-tree picker for staging and unstaging.
//!
//! Parses `git status --porcelain`, builds the list of changed files, launches
//! the full-screen TUI from [`crate::ui::stage`], then applies the user's
//! choices via `git add` / `git restore --staged` / `git restore`.

use anyhow::{Context, Result};

use crate::commands::git::{self as gitcmd, git_output};
use crate::commands::Error as CommandError;
use crate::config;
use crate::ui;
use crate::ui::stage::{run as run_tui, run_inline, StageEntry};

/// Entry point for `g stage`.
///
/// Parses the working-tree status, opens the staging TUI, and applies the
/// user's selections.
pub fn stage() -> Result<()> {
    if !gitcmd::is_inside_git_repo() {
        return Err(CommandError::NotInRepo.into());
    }

    let cfg = config::load().unwrap_or_default();

    // Resolve the repository root so all paths are consistently repo-root-
    // relative.  `git status --porcelain` always emits repo-root-relative
    // paths, but `git add` / `git restore` interpret paths relative to CWD.
    // Using `-C <root>` ensures the two agree regardless of where the user
    // invoked the command.
    let root = gitcmd::repo_root()?;

    // ── Parse git status ─────────────────────────────────────────────────────
    //
    // Do NOT use git_output_lossy — it trims the whole output, stripping the
    // leading space from the first line's index-status column.
    let raw_out = std::process::Command::new(gitcmd::git_exe())
        .args(["-C", &root, "status", "--porcelain"])
        .output()
        .context("Failed to run `git status --porcelain`")?;

    let raw = String::from_utf8_lossy(&raw_out.stdout);

    if raw.trim().is_empty() {
        ui::print_blank();
        ui::print_info("Nothing to stage — working tree is clean.");
        ui::print_blank();
        return Ok(());
    }

    // ── Build entry list ──────────────────────────────────────────────────────
    let mut entries: Vec<StageEntry> = Vec::new();

    for line in raw.lines() {
        if line.len() < 3 {
            continue;
        }
        let x = line[0..1].to_string();
        let y = line[1..2].to_string();

        // Resolve the path.  For renames the porcelain line is "R  old -> new"
        // in v1 format (or separated by NUL in v2).  We use v1 and take
        // everything after the first arrow if present.
        let raw_path = line[3..].trim();
        let path = if let Some(arrow) = raw_path.find(" -> ") {
            raw_path[arrow + 4..].to_string()
        } else {
            unquote_path(raw_path)
        };

        // Skip ignored entries.
        if x == "!" && y == "!" {
            continue;
        }

        let is_staged = x != " " && x != "?" && x != "!";
        let is_untracked = x == "?" && y == "?";

        // Include files that have any tracked change (staged or unstaged) or
        // are untracked.  Skip files that are clean in both columns.
        if is_staged || (y != " " && y != "?" && y != "!") || is_untracked {
            entries.push(StageEntry {
                path,
                x,
                y,
                is_staged,
                is_untracked,
            });
        }
    }

    if entries.is_empty() {
        ui::print_blank();
        ui::print_info("Nothing to stage — all changes are already committed.");
        ui::print_blank();
        return Ok(());
    }

    // ── Launch picker (inline or full-screen based on prompt_mode) ───────────
    let picker = if ui::is_inline_prompts() {
        run_inline(entries, cfg.stage.confirm_revert)
    } else {
        run_tui(entries, cfg.stage.confirm_revert)
    };
    let Some(result) = picker else {
        ui::print_blank();
        ui::print_info("Cancelled — no changes made.");
        ui::print_blank();
        return Ok(());
    };

    // ── Apply revert first (before staging) ───────────────────────────────────
    if !result.to_revert.is_empty() {
        let count = result.to_revert.len();
        let pb = ui::spinner(&format!(
            "Reverting {} file{}…",
            count,
            if count == 1 { "" } else { "s" }
        ));
        let mut args = vec![
            "-C".to_string(),
            root.clone(),
            "restore".to_string(),
            "--".to_string(),
        ];
        args.extend(result.to_revert.iter().cloned());
        match git_output(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>()) {
            Ok(_) => ui::spinner_success(
                pb,
                &format!(
                    "Reverted {}  {}",
                    ui::warning_bold(&count.to_string()),
                    if count == 1 { "file" } else { "files" }
                ),
            ),
            Err(e) => ui::spinner_error(pb, &format!("Revert failed: {e}")),
        }
    }

    // ── Unstage ───────────────────────────────────────────────────────────────
    if !result.to_unstage.is_empty() {
        let count = result.to_unstage.len();
        let pb = ui::spinner(&format!(
            "Unstaging {} file{}…",
            count,
            if count == 1 { "" } else { "s" }
        ));
        let mut args = vec![
            "-C".to_string(),
            root.clone(),
            "restore".to_string(),
            "--staged".to_string(),
            "--".to_string(),
        ];
        args.extend(result.to_unstage.iter().cloned());
        match git_output(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>()) {
            Ok(_) => ui::spinner_success(
                pb,
                &format!(
                    "Unstaged {}  {}",
                    ui::warning_bold(&count.to_string()),
                    if count == 1 { "file" } else { "files" }
                ),
            ),
            Err(e) => ui::spinner_error(pb, &format!("Unstage failed: {e}")),
        }
    }

    // ── Stage ─────────────────────────────────────────────────────────────────
    if !result.to_stage.is_empty() {
        let count = result.to_stage.len();
        let pb = ui::spinner(&format!(
            "Staging {} file{}…",
            count,
            if count == 1 { "" } else { "s" }
        ));
        let mut args = vec![
            "-C".to_string(),
            root.clone(),
            "add".to_string(),
            "--".to_string(),
        ];
        args.extend(result.to_stage.iter().cloned());
        match git_output(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>()) {
            Ok(_) => {
                ui::spinner_success(
                    pb,
                    &format!(
                        "Staged {}  {}",
                        ui::warning_bold(&count.to_string()),
                        if count == 1 { "file" } else { "files" }
                    ),
                );
            }
            Err(e) => ui::spinner_error(pb, &format!("Stage failed: {e}")),
        }
    }

    // ── Summary ───────────────────────────────────────────────────────────────
    if result.to_stage.is_empty() && result.to_unstage.is_empty() && result.to_revert.is_empty() {
        ui::print_blank();
        ui::print_info("No changes — selection was already in the desired state.");
    } else {
        ui::print_blank();
        ui::print_tip(&format!(
            "{}  commit staged changes",
            ui::warning(&format!("{} commit", crate::bin_name()))
        ));
    }
    ui::print_blank();

    Ok(())
}

/// Strip git's double-quote wrapping from a path if present.
fn unquote_path(p: &str) -> String {
    if p.starts_with('"') && p.ends_with('"') && p.len() >= 2 {
        p[1..p.len() - 1].to_string()
    } else {
        p.to_string()
    }
}
