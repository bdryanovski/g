//! `g add` — interactive multi-select stager/unstager + dispatch helper.
//!
//! With no positional arguments, [`dispatch_add`] launches [`interactive_add`],
//! which presents two sequential pickers (unstage, then stage).  When the user
//! passes any path or flag, the call is forwarded straight to `git add`.

use anyhow::{Context, Result};
use std::process::Command;

use crate::commands::Error as CommandError;
use crate::ui;

use super::dry_run::{dry_run_action, is_dry_run};
use super::exec::{git_exe, git_output, passthrough};
use super::repo::{is_inside_git_repo, repo_root};

/// Route `g add [...]`: with no positional arguments, launch the interactive
/// stager; otherwise forward everything to `git add`.
pub fn dispatch_add(args: crate::cli::GitPassArgs) -> Result<()> {
    if args.args.is_empty() {
        interactive_add()
    } else {
        let mut git_args = vec!["add".to_string()];
        git_args.extend(args.args);
        passthrough(&git_args)
    }
}

/// Present full-screen ratatui pickers to stage and/or unstage files.
///
/// Two sequential screens are shown (each only when relevant):
///
/// 1. **Unstage picker** — shows files that are staged (index column X is
///    non-blank); selecting them runs `git restore --staged <file>`.
/// 2. **Stage picker** — shows untracked and working-tree-modified files
///    (column Y is non-blank); selecting them runs `git add <file>`.
pub fn interactive_add() -> Result<()> {
    if !is_inside_git_repo() {
        return Err(CommandError::NotInRepo.into());
    }

    if is_dry_run() {
        dry_run_action(
            "git add / restore --staged <interactive>",
            "Launch interactive picker to stage or unstage files",
        );
        return Ok(());
    }

    // Resolve the repository root so all paths are consistently repo-root-
    // relative.  `git status --porcelain` always emits repo-root-relative
    // paths, but `git add` / `git restore` interpret paths relative to CWD.
    // Using `-C <root>` ensures the two agree.
    let root = repo_root()?;

    // Fetch raw porcelain output without trimming (the leading space on the first
    // line carries the index status and must be preserved).
    let raw_out = Command::new(git_exe())
        .args(["-C", &root, "status", "--porcelain"])
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

            let mut git_args = vec!["-C", &root, "restore", "--staged", "--"];
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

            let mut git_args = vec!["-C", &root, "add", "--"];
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
