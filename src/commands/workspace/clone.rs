//! `g clone --workspace <url>` — clone a remote repo into the container
//! layout (the inner worktree is named after the detected default branch).
//!
//! This entry point is called from `main::run` **before** clap parsing so the
//! `--workspace` flag can be stripped and the remaining args forwarded to
//! `git clone`.

use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::fs;

use crate::commands::git as gitcmd;
use crate::commands::Ctx;
use crate::storage::{repos, stats, workspaces as ws_store};
use crate::ui;

/// Clone a remote repository and set it up in the container/worktree layout.
///
/// 1. Queries the remote for its default branch via `git ls-remote --symref`.
/// 2. Creates a container directory named after the repo.
/// 3. Clones into `<container>/<default_branch>`.
pub fn run(ctx: &Ctx, args: &[String]) -> Result<()> {
    let conn = ctx.conn;
    let url = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .with_context(|| "No URL found. Usage: g clone <url> [dest] --workspace")?;

    let repo_name = url
        .trim_end_matches('/')
        .split('/')
        .next_back()
        .unwrap_or("repo")
        .trim_end_matches(".git")
        .to_string();

    let non_flag_args: Vec<&str> = args
        .iter()
        .filter(|a| !a.starts_with('-'))
        .map(|s| s.as_str())
        .collect();
    let container_name = non_flag_args.get(1).copied().unwrap_or(repo_name.as_str());

    let cwd = std::env::current_dir()?;
    let container_dir = cwd.join(container_name);

    if container_dir.exists() {
        bail!(
            "Directory '{}' already exists. Remove it or use a different destination.",
            container_dir.display()
        );
    }

    let default_branch = detect_remote_default_branch(url).unwrap_or_else(|_| "main".to_string());

    let inner_dir = container_dir.join(&default_branch);
    let inner_dir_str = inner_dir.to_string_lossy().to_string();

    fs::create_dir_all(&container_dir).with_context(|| {
        format!(
            "Failed to create container directory '{}'",
            container_dir.display()
        )
    })?;

    let mut clone_args: Vec<&str> = vec!["clone"];
    for a in args {
        if a.starts_with('-') {
            clone_args.push(a.as_str());
        }
    }
    clone_args.push(url.as_str());
    clone_args.push(&inner_dir_str);

    let result = gitcmd::git_mutate(
        &clone_args,
        &format!("Clone '{}' into '{}'", url, inner_dir_str),
    );

    if let Err(e) = result {
        let _ = fs::remove_dir_all(&container_dir);
        return Err(e).context("Clone failed");
    }

    if !gitcmd::is_dry_run() {
        let container_root_str = container_dir.to_string_lossy().to_string();
        let repo_id = repos::upsert(conn, &container_root_str)?;

        ws_store::set_container_root(conn, repo_id, &container_root_str)?;

        if ws_store::find_by_path(conn, &inner_dir_str)?.is_none() {
            ws_store::insert(
                conn,
                repo_id,
                &ws_store::NewWorkspace {
                    name: &default_branch,
                    description: None,
                    path: &inner_dir_str,
                    branch: &default_branch,
                    container_root: Some(&container_root_str),
                    created_at: Utc::now(),
                },
            )?;
        }

        stats::record_workspace_event(conn, None, Some(repo_id), "clone").ok();

        ui::print_blank();
        ui::print_success(&format!(
            "Cloned '{}' into container workspace at {}",
            url,
            ui::primary(&container_dir.display().to_string())
        ));
        ui::print_line(&format!(
            "     {} {}",
            ui::muted("main workspace:"),
            ui::link_primary_bold(&inner_dir_str)
        ));
        ui::print_blank();
        ui::print_indented(&format!(
            "{} cd {}",
            ui::muted("next:"),
            ui::success(&inner_dir.display().to_string())
        ));
        ui::print_blank();
    }

    Ok(())
}

/// Query the remote for its default branch via `git ls-remote --symref`.
fn detect_remote_default_branch(url: &str) -> Result<String> {
    let output = gitcmd::git_output(&["ls-remote", "--symref", url, "HEAD"])
        .context("Failed to query remote default branch")?;

    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("ref: refs/heads/") {
            let branch = rest.split('\t').next().unwrap_or("").trim();
            if !branch.is_empty() {
                return Ok(branch.to_string());
            }
        }
    }

    bail!("Could not determine default branch from remote HEAD")
}
