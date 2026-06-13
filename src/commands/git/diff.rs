//! `g diff` — pipe `git diff` through an external tool when configured.
//!
//! Selects the tool from `[diff].tool`:
//! - `"auto"` → detect `delta` or `diff-so-fancy` in `$PATH`.
//! - `"delta"` / `"diff-so-fancy"` → pipe `git diff` output through the tool.
//! - Anything else → forward directly to `git diff`.

use anyhow::{Context, Result};
use std::process::{Command, Stdio};

use crate::config;

use super::exec::{git_exe, passthrough};

/// Run diff using a configured external tool if available, otherwise passthrough.
pub fn enhanced_diff(extra_args: &[String]) -> Result<()> {
    let cfg = config::load().unwrap_or_default();
    let tool = resolve_diff_tool(&cfg.diff.tool);

    match tool.as_str() {
        "delta" => {
            if which::which("delta").is_ok() {
                let output = Command::new(git_exe())
                    .args(["diff"])
                    .args(extra_args)
                    .stdout(Stdio::piped())
                    .spawn()?
                    .stdout
                    .context("no stdout")?;

                Command::new("delta").stdin(output).status()?;
                return Ok(());
            }
            passthrough_with_subcommand("diff", extra_args)
        }
        "diff-so-fancy" => {
            if which::which("diff-so-fancy").is_ok() {
                let output = Command::new(git_exe())
                    .args(["diff", "--color=always"])
                    .args(extra_args)
                    .stdout(Stdio::piped())
                    .spawn()?
                    .stdout
                    .context("no stdout")?;

                Command::new("diff-so-fancy").stdin(output).status()?;
                return Ok(());
            }
            passthrough_with_subcommand("diff", extra_args)
        }
        _ => passthrough_with_subcommand("diff", extra_args),
    }
}

/// Determine which diff tool to use based on the config value and `$PATH`.
fn resolve_diff_tool(tool: &str) -> String {
    match tool {
        "auto" => {
            if which::which("delta").is_ok() {
                "delta".to_string()
            } else if which::which("diff-so-fancy").is_ok() {
                "diff-so-fancy".to_string()
            } else {
                "builtin".to_string()
            }
        }
        other => other.to_string(),
    }
}

/// Prepend a subcommand name to `extra` and delegate to [`passthrough`].
fn passthrough_with_subcommand(sub: &str, extra: &[String]) -> Result<()> {
    let mut args = vec![sub.to_string()];
    args.extend_from_slice(extra);
    passthrough(&args)
}
