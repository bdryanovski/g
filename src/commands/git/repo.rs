//! Repo-introspection helpers: current branch, repo root, default branch,
//! and a TTY-quiet "are we inside a repo?" check.

use anyhow::Result;
use std::process::{Command, Stdio};

use crate::config;

use super::exec::{git_exe, git_output, git_output_lossy};

/// Return the name of the currently checked-out branch.
///
/// Returns `"HEAD"` when in detached-HEAD state.
pub fn current_branch() -> Result<String> {
    git_output(&["rev-parse", "--abbrev-ref", "HEAD"])
}

/// Return the absolute path of the repository root directory.
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
