//! Low-level wrappers around the `git` binary.
//!
//! These functions spawn `git` directly via [`std::process::Command`] and are
//! the only place in the codebase that does so.  Everything else goes through
//! [`git_output`] or [`passthrough`].

use anyhow::{bail, Context, Result};
use std::process::{Command, Stdio};

use crate::config;

/// Resolve the git executable path from config, falling back to `"git"`.
///
/// Reads `[general].git_path` from the user config.  If the config cannot be
/// loaded or the key is absent, `"git"` is returned so the OS resolves it via
/// `$PATH`.
pub fn git_exe() -> String {
    let cfg = config::load().unwrap_or_default();
    cfg.general.git_path.unwrap_or_else(|| "git".to_string())
}

/// Run `git` with `args` and return stdout as a trimmed `String`.
///
/// Stderr from git is captured and returned as the error message on non-zero
/// exit so callers get the same diagnostic git would normally print.
pub fn git_output(args: &[&str]) -> Result<String> {
    let out = Command::new(git_exe())
        .args(args)
        .output()
        .with_context(|| format!("Failed to run git {:?}", args))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        bail!("{}", stderr)
    }
}

/// Returns `true` if `ancestor` is reachable from `descendant`'s history.
///
/// Internally runs `git merge-base --is-ancestor <ancestor> <descendant>`.
pub fn is_ancestor(ancestor: &str, descendant: &str) -> Result<bool> {
    let status = Command::new(git_exe())
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .status()
        .with_context(|| {
            format!(
                "Failed to run git merge-base --is-ancestor {} {}",
                ancestor, descendant
            )
        })?;
    if status.success() {
        Ok(true)
    } else if status.code() == Some(1) {
        Ok(false)
    } else {
        bail!("git merge-base --is-ancestor exited with {:?}", status);
    }
}

/// Returns `true` when `git status --porcelain` produces no output (clean tree).
pub fn working_tree_clean() -> Result<bool> {
    let s = git_output(&["status", "--porcelain"])?;
    Ok(s.is_empty())
}

/// Bail with a standardised message if the working tree has uncommitted changes.
pub fn require_clean_tree(operation: &str) -> Result<()> {
    if !working_tree_clean()? {
        bail!(
            "Working tree is not clean. Commit or stash changes before {}.",
            operation
        );
    }
    Ok(())
}

/// Run `git` with `args` and return stdout, ignoring a non-zero exit.
///
/// Non-UTF-8 bytes in the output are replaced with the Unicode replacement
/// character (`U+FFFD`).
pub fn git_output_lossy(args: &[&str]) -> String {
    Command::new(git_exe())
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Stream a git invocation directly to the terminal (stdin/stdout/stderr inherited).
///
/// Used for "passthrough" commands where we want git's own interactive output,
/// pager, colour handling, etc.  If git exits non-zero, this function calls
/// [`std::process::exit`] with that code so the shell receives the correct
/// exit status.
///
/// Alias resolution happens here: a first-argument that matches a configured
/// alias is expanded before forwarding to git.
///
/// In dry-run mode the command is printed but not executed.
pub fn passthrough(args: &[String]) -> Result<()> {
    let cfg = config::load().unwrap_or_default();

    // Check aliases first so `g co` works even as a passthrough.
    if let Some(first) = args.first() {
        if let Some(alias_target) = cfg.aliases.get(first) {
            let mut new_args: Vec<String> =
                alias_target.split_whitespace().map(String::from).collect();
            new_args.extend_from_slice(&args[1..]);
            return passthrough(&new_args);
        }
    }

    if super::dry_run::is_dry_run() {
        let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        super::dry_run::print_dry_run_git(&str_args, "Passthrough — forwarded to git as-is");
        return Ok(());
    }

    let status = Command::new(git_exe())
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("Failed to execute git")?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}
