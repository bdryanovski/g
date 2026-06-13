//! `g workspace init` — reorganise an existing, non-worktree repo into the
//! container layout.
//!
//! Given a repo at `/path/to/repo` on branch `main`, the operation is:
//!
//! ```text
//! mv  /path/to/repo          /path/to/repo--ws-tmp  (temp rename)
//! mkdir                      /path/to/repo           (container)
//! mv  /path/to/repo--ws-tmp  /path/to/repo/main      (inner worktree)
//! ```
//!
//! The store is updated before `git worktree repair` runs so that a repair
//! failure does not leave the filesystem and metadata out of sync.

use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;

use crate::commands::git as gitcmd;
use crate::commands::Ctx;
use crate::config;
use crate::storage::{stats, workspaces as ws_store};
use crate::ui;

use super::shared::current_repo_id;

pub(super) fn run(ctx: &Ctx) -> Result<()> {
    let conn = ctx.conn;
    let repo_root_str =
        gitcmd::repo_root().context("Not inside a git repository. Run `git init` first.")?;
    let repo_root = PathBuf::from(&repo_root_str);

    let parent = repo_root
        .parent()
        .context("Repository is at the filesystem root — cannot create container.")?;
    let repo_name = repo_root
        .file_name()
        .context("Could not determine repository name from path")?
        .to_string_lossy()
        .to_string();

    // Detect the default branch name.
    let branch_name = gitcmd::git_output(&["symbolic-ref", "--short", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| {
            config::load()
                .map(|c| c.general.default_branch.clone())
                .unwrap_or_else(|_| "main".to_string())
        });

    // Guard: if the current repo is already inside a known container, block.
    let repo_id = current_repo_id(conn)?;
    if ws_store::get_container_root(conn, repo_id)?.is_some() {
        bail!(
            "This repository is already using the container workspace layout.\n\
             Run `{} workspace list` to see your workspaces.",
            crate::bin_name()
        );
    }

    let container_dir = repo_root.clone();
    let inner_dir = container_dir.join(&branch_name);
    let temp_name = format!("{}--ws-tmp", repo_name);
    let temp_path = parent.join(&temp_name);

    if temp_path.exists() {
        bail!(
            "Temporary path '{}' already exists. Please remove it and try again.",
            temp_path.display()
        );
    }

    // Show the plan.
    ui::print_blank();
    ui::print_indented(&format!(
        "{} workspace init",
        ui::success_bold(crate::bin_name())
    ));
    ui::print_blank();
    ui::print_indented(&format!(
        "This will reorganise '{}' into a container layout:",
        ui::primary(&repo_root.display().to_string())
    ));
    ui::print_blank();
    ui::print_line(&format!(
        "    {} {}  →  {}",
        ui::muted("move"),
        ui::warning(&repo_root.display().to_string()),
        ui::muted(&temp_path.display().to_string())
    ));
    ui::print_line(&format!(
        "    {} {}",
        ui::muted("mkdir"),
        ui::warning(&container_dir.display().to_string())
    ));
    ui::print_line(&format!(
        "    {} {}  →  {}",
        ui::muted("move"),
        ui::muted(&temp_path.display().to_string()),
        ui::success(&inner_dir.display().to_string())
    ));
    ui::print_blank();
    ui::print_indented(&format!(
        "After this, new workspaces will be created inside '{}'.",
        ui::primary(&container_dir.display().to_string())
    ));
    ui::print_blank();

    if gitcmd::is_dry_run() {
        gitcmd::dry_run_action(
            &format!("mv {} {}", repo_root.display(), temp_path.display()),
            "Move repo to temporary location",
        );
        gitcmd::dry_run_action(
            &format!("mkdir {}", container_dir.display()),
            "Create container directory",
        );
        gitcmd::dry_run_action(
            &format!("mv {} {}", temp_path.display(), inner_dir.display()),
            "Move repo into container as the main workspace",
        );
        gitcmd::dry_run_action(
            "git -C <inner> worktree repair && git -C <inner> worktree prune",
            "Repair and prune git worktree tracking after directory move",
        );
        return Ok(());
    }

    if !ui::confirm("Proceed with workspace init?", false) {
        ui::print_blank();
        ui::print_info("Cancelled.");
        ui::print_blank();
        return Ok(());
    }

    // Step out of the repo before we move it.
    std::env::set_current_dir(parent)
        .context("Failed to change directory to parent before moving repo")?;

    // Step 1: rename repo → temp.
    fs::rename(&repo_root, &temp_path).with_context(|| {
        format!(
            "Failed to move '{}' to '{}'",
            repo_root.display(),
            temp_path.display()
        )
    })?;

    // Steps 2 + 3: create container, move temp → inner.
    let move_result = (|| -> Result<()> {
        fs::create_dir_all(&container_dir).with_context(|| {
            format!(
                "Failed to create container directory '{}'",
                container_dir.display()
            )
        })?;
        fs::rename(&temp_path, &inner_dir).with_context(|| {
            format!(
                "Failed to move '{}' to '{}'",
                temp_path.display(),
                inner_dir.display()
            )
        })?;
        Ok(())
    })();

    if let Err(e) = move_result {
        if temp_path.exists() && !repo_root.exists() {
            let _ = fs::rename(&temp_path, &repo_root);
        }
        return Err(e).context("Init failed — attempted to roll back to original state");
    }

    let container_root_str = container_dir.to_string_lossy().to_string();
    let inner_dir_str = inner_dir.to_string_lossy().to_string();

    // Persist metadata BEFORE the repair step so a repair failure does not
    // leave the filesystem and metadata out of sync.
    ws_store::set_container_root(conn, repo_id, &container_root_str)?;

    if ws_store::find_by_path(conn, &inner_dir_str)?.is_none() {
        ws_store::insert(
            conn,
            repo_id,
            &ws_store::NewWorkspace {
                name: &branch_name,
                description: None,
                path: &inner_dir_str,
                branch: &branch_name,
                container_root: Some(&container_root_str),
                created_at: Utc::now(),
            },
        )?;
    }

    // Step 4: repair git worktree tracking after the directory move.
    gitcmd::git_output(&["-C", &inner_dir_str, "worktree", "repair"])
        .context("Failed to repair git worktree tracking after move")?;
    let _ = gitcmd::git_output(&["-C", &inner_dir_str, "worktree", "prune"]);

    stats::record_workspace_event(conn, None, Some(repo_id), "init").ok();

    ui::print_blank();
    ui::print_success(&format!(
        "Repository reorganised into container layout at {}",
        ui::primary(&container_dir.display().to_string())
    ));
    ui::print_key_value_pairs(&[
        ("main workspace", ui::link_primary_bold(&inner_dir_str)),
        (
            "next",
            format!("cd {}", ui::success(&inner_dir.display().to_string())),
        ),
    ]);
    ui::print_blank();

    Ok(())
}
