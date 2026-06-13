//! `g workspace create <name>` — create a new git worktree and save its
//! metadata, optionally copying untracked/gitignored files into it.

use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

use crate::commands::git as gitcmd;
use crate::commands::{Ctx, Error as CommandError};
use crate::storage::{stats, workspaces as ws_store};
use crate::ui;

use super::shared::{load_repo_workspaces, worktree_path_for};

/// Create a new worktree and save its metadata.
///
/// Branch resolution order:
/// 1. Branch exists locally → check out directly.
/// 2. Branch exists on `origin` → create a local tracking branch.
/// 3. Neither → create a new branch (optionally from `start_point`).
///
/// When `copy` is `true`, an interactive picker lets the user choose
/// untracked and gitignored files to copy into the new worktree.
pub(super) fn run(
    ctx: &Ctx,
    name: &str,
    branch: Option<&str>,
    start_point: Option<&str>,
    description: Option<&str>,
    copy: bool,
) -> Result<()> {
    let conn = ctx.conn;
    let (repo_id, workspaces) = load_repo_workspaces(conn)?;

    if workspaces.iter().any(|w| w.name == name) {
        return Err(CommandError::WorkspaceExists(name.to_string()).into());
    }

    let wt_path = worktree_path_for(conn, name)?;
    if wt_path.exists() {
        bail!(
            "Directory '{}' already exists. Choose a different workspace name.",
            wt_path.display()
        );
    }

    let branch_name = branch.unwrap_or(name);
    let wt_path_str = wt_path.to_string_lossy().to_string();

    let local_exists = gitcmd::git_output(&["rev-parse", "--verify", branch_name]).is_ok();
    let remote_ref = format!("origin/{}", branch_name);
    let remote_exists =
        !local_exists && gitcmd::git_output(&["rev-parse", "--verify", &remote_ref]).is_ok();

    let result = if local_exists {
        gitcmd::git_mutate(
            &["worktree", "add", &wt_path_str, branch_name],
            &format!(
                "Create worktree for existing local branch '{}' at {}",
                branch_name, wt_path_str
            ),
        )
    } else if remote_exists {
        gitcmd::git_mutate(
            &[
                "worktree",
                "add",
                &wt_path_str,
                "-b",
                branch_name,
                "--track",
                &remote_ref,
            ],
            &format!(
                "Create worktree tracking remote branch '{}' at {}",
                branch_name, wt_path_str
            ),
        )
    } else {
        let mut args = vec!["worktree", "add", "-b", branch_name, &wt_path_str];
        if let Some(sp) = start_point {
            args.push(sp);
        }
        gitcmd::git_mutate(
            &args,
            &format!(
                "Create new branch '{}' and worktree at {}",
                branch_name, wt_path_str
            ),
        )
    };

    result.with_context(|| {
        format!(
            "Failed to create worktree at '{}' for branch '{}'",
            wt_path.display(),
            branch_name
        )
    })?;

    if !gitcmd::is_dry_run() {
        let container_root = ws_store::get_container_root(conn, repo_id)?;
        let ws_id = ws_store::insert(
            conn,
            repo_id,
            &ws_store::NewWorkspace {
                name,
                description,
                path: &wt_path_str,
                branch: branch_name,
                container_root: container_root.as_deref(),
                created_at: Utc::now(),
            },
        )?;
        stats::record_workspace_event(conn, Some(ws_id), Some(repo_id), "create").ok();
        stats::record_branch_event(conn, repo_id, branch_name, "create").ok();
    } else {
        gitcmd::dry_run_action(
            "Save workspace metadata",
            &format!(
                "Register workspace '{}' on branch '{}' in g.db",
                name, branch_name
            ),
        );
    }

    if copy && !gitcmd::is_dry_run() {
        copy_untracked_files(&wt_path)?;
    } else if copy {
        gitcmd::dry_run_action(
            "Copy untracked files",
            "Show interactive picker and copy selected files to new worktree",
        );
    }

    if !gitcmd::is_dry_run() {
        ui::print_blank();
        ui::print_success(&format!(
            "Created workspace {} on branch {}",
            ui::success_bold(name),
            ui::primary(branch_name)
        ));
        ui::print_key_value_pairs(&[("path", ui::link_primary_bold(&wt_path_str))]);
        ui::print_blank();
        ui::print_tip(&format!(
            "{} workspace switch {}  to open a shell there",
            crate::bin_name(),
            name
        ));
        ui::print_blank();
    }

    Ok(())
}

/// Present an interactive checklist of untracked and gitignored files and copy
/// the user's selection into `dest`.
fn copy_untracked_files(dest: &Path) -> Result<()> {
    let untracked_out =
        gitcmd::git_output(&["ls-files", "--others", "--exclude-standard"]).unwrap_or_default();

    let ignored_out = gitcmd::git_output(&[
        "ls-files",
        "--others",
        "-i",
        "--exclude-standard",
        "--directory",
    ])
    .unwrap_or_default();

    let mut candidates: Vec<String> = untracked_out
        .lines()
        .chain(ignored_out.lines())
        .filter(|l| !l.is_empty())
        .map(|l| l.trim_end_matches('/').to_string())
        .collect();
    candidates.dedup();

    if candidates.is_empty() {
        return Ok(());
    }

    ui::print_blank();
    let options: Vec<ui::SelectOption> = candidates
        .iter()
        .map(|c| ui::SelectOption::new(c.clone()))
        .collect();
    let selected = ui::multi_select("Copy files to new workspace", &options);
    if selected.is_empty() {
        return Ok(());
    }

    let repo_root = gitcmd::repo_root()?;
    let src_root = PathBuf::from(&repo_root);

    let total_files: u64 = selected
        .iter()
        .map(|&idx| count_files(&src_root.join(&candidates[idx])))
        .sum();

    let pb = ui::progress_bar(
        total_files.max(1),
        &format!(
            "Copying {} item{}…",
            selected.len(),
            if selected.len() == 1 { "" } else { "s" }
        ),
    );

    for idx in &selected {
        let rel = &candidates[*idx];
        let src = src_root.join(rel);
        let dst = dest.join(rel);
        copy_path(&src, &dst, &pb).with_context(|| {
            format!("Failed to copy '{}' to '{}'", src.display(), dst.display())
        })?;
    }

    pb.finish_and_clear();
    ui::print_success(&format!(
        "Copied {} item{}",
        selected.len(),
        if selected.len() == 1 { "" } else { "s" },
    ));

    Ok(())
}

/// Recursively count the number of regular files under `path`.
fn count_files(path: &Path) -> u64 {
    if path.is_file() {
        return 1;
    }
    if path.is_dir() {
        return fs::read_dir(path)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| count_files(&e.path()))
                    .sum()
            })
            .unwrap_or(0);
    }
    0
}

/// Copy `src` to `dst`, handling both regular files and directories.
fn copy_path(src: &Path, dst: &Path, pb: &ui::ProgressBar) -> Result<()> {
    if src.is_dir() {
        fs::create_dir_all(dst)
            .with_context(|| format!("Failed to create directory '{}'", dst.display()))?;
        for entry in fs::read_dir(src)
            .with_context(|| format!("Failed to read directory '{}'", src.display()))?
        {
            let entry =
                entry.with_context(|| format!("Failed to read entry in '{}'", src.display()))?;
            copy_path(&entry.path(), &dst.join(entry.file_name()), pb)?;
        }
    } else {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory '{}'", parent.display()))?;
        }
        let label = src
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        pb.set_message(label);
        fs::copy(src, dst).with_context(|| format!("Failed to copy file '{}'", src.display()))?;
        pb.inc(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::TestRepo;

    #[test]
    fn creates_worktree_directory_and_db_row() {
        let repo = TestRepo::new();
        let ctx = repo.ctx();

        run(&ctx, "feature-x", None, None, Some("a new feature"), false)
            .expect("create should succeed");

        // The DB row exists with the right branch + description.
        let (repo_id, list) = load_repo_workspaces(&repo.conn).unwrap();
        // `load_repo_workspaces` also reconciles in the main worktree, so we
        // expect at least our new one in the list.
        let created = list
            .iter()
            .find(|w| w.name == "feature-x")
            .expect("feature-x row");
        assert_eq!(created.branch, "feature-x");
        assert_eq!(created.description.as_deref(), Some("a new feature"));
        assert_eq!(created.repo_id, repo_id);

        // The worktree directory exists on disk at the sibling path.
        let wt_path = PathBuf::from(&created.path);
        assert!(
            wt_path.is_dir(),
            "worktree dir should exist: {}",
            wt_path.display()
        );
        assert!(
            wt_path.join(".git").exists(),
            "worktree should be a real git worktree"
        );

        // The branch is now visible to git.
        let branches = repo.git(&["branch", "--list", "feature-x"]);
        assert!(
            !branches.is_empty(),
            "git should list the new branch: {branches:?}"
        );
    }

    #[test]
    fn rejects_duplicate_name_without_touching_disk() {
        let repo = TestRepo::new();
        let ctx = repo.ctx();
        run(&ctx, "dup", None, None, None, false).unwrap();
        let err = run(&ctx, "dup", None, None, None, false)
            .expect_err("second create with same name must fail");
        match err.downcast_ref::<CommandError>() {
            Some(CommandError::WorkspaceExists(n)) => assert_eq!(n, "dup"),
            other => panic!("expected WorkspaceExists, got {other:?}"),
        }
    }
}
