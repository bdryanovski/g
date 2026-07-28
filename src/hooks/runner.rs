//! Hook command execution engine.
//!
//! Executes hook commands with proper environment, output capture,
//! and timing display using the TaskRunner UI component.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Instant;

use anyhow::{Context, Result};

use crate::config::hooks::{HookCommand, HookConfig, HookType};
use crate::ui::{self, TaskRunner};

use super::staged::filter_by_patterns;

/// Result of running a hook.
#[derive(Debug)]
pub enum HookResult {
    /// All commands passed.
    Success,
    /// No commands configured for this hook.
    NoCommands,
    /// A command failed (includes command name and error message).
    Failed { command: String, message: String },
    /// Skipped due to no matching staged files.
    Skipped,
}

/// Environment variables passed to hook commands.
pub struct HookEnv {
    /// The hook type being run.
    pub hook_type: HookType,
    /// Repository root path.
    pub repo_root: String,
    /// Currently staged files.
    pub staged_files: Vec<String>,
    /// Current branch name.
    pub branch: String,
}

impl HookEnv {
    /// Create a new hook environment.
    pub fn new(hook_type: HookType) -> Result<Self> {
        let repo_root = crate::commands::git::repo_root()?;
        let staged_files = super::staged::get_staged_files().unwrap_or_default();
        let branch = crate::commands::git::current_branch().unwrap_or_default();

        Ok(Self {
            hook_type,
            repo_root,
            staged_files,
            branch,
        })
    }

    /// Convert to environment variable map for subprocess.
    fn to_env_vars(&self) -> Vec<(String, String)> {
        vec![
            (
                "G_HOOK_NAME".to_string(),
                self.hook_type.as_str().to_string(),
            ),
            ("G_REPO_ROOT".to_string(), self.repo_root.clone()),
            ("G_STAGED_FILES".to_string(), self.staged_files.join("\n")),
            ("G_BRANCH".to_string(), self.branch.clone()),
        ]
    }
}

/// Run all commands for a hook configuration.
///
/// Returns early if any command fails (for blocking hooks).
pub fn run_hook(config: &HookConfig, env: &HookEnv, dry_run: bool) -> Result<HookResult> {
    if !config.enabled || config.commands.is_empty() {
        return Ok(HookResult::NoCommands);
    }

    // Check if we should run based on staged files
    if !config.staged_patterns.is_empty() {
        let matching = filter_by_patterns(&env.staged_files, &config.staged_patterns);
        if matching.is_empty() {
            return Ok(HookResult::Skipped);
        }
    }

    for cmd in &config.commands {
        let result = run_command(cmd, config, env, dry_run)?;
        if let HookResult::Failed { .. } = result {
            return Ok(result);
        }
    }

    Ok(HookResult::Success)
}

/// Run a single hook command.
fn run_command(
    cmd: &HookCommand,
    hook_config: &HookConfig,
    env: &HookEnv,
    dry_run: bool,
) -> Result<HookResult> {
    let display_name = cmd.display_name();

    // Determine which staged files to pass (if any)
    let patterns = cmd
        .staged_patterns
        .as_ref()
        .unwrap_or(&hook_config.staged_patterns);
    let pass_files = cmd.pass_files.unwrap_or(hook_config.pass_files);

    let matching_files = filter_by_patterns(&env.staged_files, patterns);

    // Skip if patterns are specified but no files match
    if !patterns.is_empty() && matching_files.is_empty() {
        return Ok(HookResult::Skipped);
    }

    // Build the full command with files if configured
    let full_command = if pass_files && !matching_files.is_empty() {
        format!("{} {}", cmd.run, matching_files.join(" "))
    } else {
        cmd.run.clone()
    };

    if dry_run {
        ui::print_info(&format!("Would run: {}", full_command));
        return Ok(HookResult::Success);
    }

    // Create the task runner display
    let start = Instant::now();

    // Print the start line
    let runner = TaskRunner::new(env.hook_type.as_str(), display_name);
    runner.print_start();

    // Determine working directory
    let cwd = cmd
        .cwd
        .as_ref()
        .map(|c| std::path::PathBuf::from(&env.repo_root).join(c))
        .unwrap_or_else(|| std::path::PathBuf::from(&env.repo_root));

    // Execute the command
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&full_command)
        .current_dir(&cwd)
        .envs(env.to_env_vars())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to execute: {}", full_command))?;

    // Stream stdout
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let mut output_lines = Vec::new();

    if let Some(stdout) = stdout {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            TaskRunner::print_output_line(&line);
            output_lines.push(line);
        }
    }

    // Capture stderr (show after stdout)
    if let Some(stderr) = stderr {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            TaskRunner::print_output_line(&line);
            output_lines.push(line);
        }
    }

    let status = child.wait().context("Failed to wait for command")?;
    let duration = start.elapsed();

    // Print the completion line
    let final_runner = TaskRunner::new(env.hook_type.as_str(), display_name).duration(duration);

    if status.success() {
        final_runner.print_done();
        Ok(HookResult::Success)
    } else {
        final_runner.failed().print_done();
        Ok(HookResult::Failed {
            command: display_name.to_string(),
            message: format!("Exit code: {}", status.code().unwrap_or(-1)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_result_variants() {
        let success = HookResult::Success;
        assert!(matches!(success, HookResult::Success));

        let failed = HookResult::Failed {
            command: "test".to_string(),
            message: "error".to_string(),
        };
        assert!(matches!(failed, HookResult::Failed { .. }));
    }
}
