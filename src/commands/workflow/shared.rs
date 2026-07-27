//! Shared utilities for workflow commands.

use anyhow::{bail, Context, Result};

use crate::config;
use crate::config::workflow::{BranchType, MergeStrategy, Workflow, WorkflowsConfig};
use crate::config::workflow_presets;
use crate::commands::git::{git_output, require_clean_tree};

/// Load the effective workflow configuration.
///
/// Merges global config with repo-local overrides and built-in presets.
pub fn load_workflows() -> Result<WorkflowsConfig> {
    // First try to load user config
    let mut workflows = config::load_workflows().unwrap_or_default();

    // If no workflows defined, use presets
    if workflows.is_empty() {
        workflows = workflow_presets::all_presets();
    }

    Ok(workflows)
}

/// Get the active workflow.
///
/// Returns the workflow set as default, or the first available.
pub fn get_active_workflow() -> Result<(String, Workflow)> {
    let workflows = load_workflows()?;

    if let Some((name, workflow)) = workflows.active() {
        return Ok((name.clone(), workflow.clone()));
    }

    // Fallback to github-flow preset
    if let Some(workflow) = workflow_presets::get_preset("github-flow") {
        return Ok(("github-flow".to_string(), workflow));
    }

    bail!("No workflow configured. Run `g workflow init` to set up.")
}

/// Get a specific workflow by name.
///
/// Checks user config first, then built-in presets.
pub fn get_workflow(name: &str) -> Result<Workflow> {
    let workflows = load_workflows()?;

    if let Some(workflow) = workflows.get(name) {
        return Ok(workflow.clone());
    }

    if let Some(workflow) = workflow_presets::get_preset(name) {
        return Ok(workflow);
    }

    bail!(
        "Workflow '{}' not found. Run `g workflow list` to see available workflows.",
        name
    )
}

/// Get the current git branch name.
pub fn current_branch() -> Result<String> {
    git_output(&["branch", "--show-current"])
        .context("Failed to get current branch")
}

/// Check if a branch exists locally.
pub fn branch_exists(name: &str) -> Result<bool> {
    let result = git_output(&["rev-parse", "--verify", &format!("refs/heads/{}", name)]);
    Ok(result.is_ok())
}

/// Check if a branch exists on the remote.
pub fn remote_branch_exists(name: &str) -> Result<bool> {
    let result = git_output(&["rev-parse", "--verify", &format!("refs/remotes/origin/{}", name)]);
    Ok(result.is_ok())
}

/// Get the merge-base between two branches.
#[allow(dead_code)]
pub fn merge_base(branch1: &str, branch2: &str) -> Result<String> {
    git_output(&["merge-base", branch1, branch2])
        .with_context(|| format!("Failed to find merge-base between {} and {}", branch1, branch2))
}

/// Count commits ahead/behind between two refs.
pub fn commits_ahead_behind(branch: &str, base: &str) -> Result<(usize, usize)> {
    let output = git_output(&["rev-list", "--left-right", "--count", &format!("{}...{}", base, branch)])?;
    let parts: Vec<&str> = output.split_whitespace().collect();
    if parts.len() != 2 {
        bail!("Unexpected output from rev-list: {}", output);
    }
    let behind = parts[0].parse().unwrap_or(0);
    let ahead = parts[1].parse().unwrap_or(0);
    Ok((ahead, behind))
}

/// Detect the branch type from a branch name using workflow configuration.
pub fn detect_branch_type<'a>(workflow: &'a Workflow, branch: &str) -> Option<&'a BranchType> {
    workflow.type_for_branch(branch)
}

/// Create a full branch name from type and name.
pub fn make_branch_name(branch_type: &BranchType, name: &str) -> String {
    if branch_type.prefix.is_empty() {
        name.to_string()
    } else {
        format!("{}{}", branch_type.prefix, name)
    }
}

/// Extract the name portion from a full branch name (without prefix).
pub fn extract_branch_name(branch_type: &BranchType, full_name: &str) -> String {
    if branch_type.prefix.is_empty() {
        full_name.to_string()
    } else {
        full_name.strip_prefix(&branch_type.prefix)
            .unwrap_or(full_name)
            .to_string()
    }
}

/// Resolve the source branch for a branch type.
///
/// Handles special values like "HEAD", "main", "develop", and patterns.
pub fn resolve_source_branch(workflow: &Workflow, branch_type: &BranchType) -> Result<String> {
    let source = &branch_type.source;

    if source == "HEAD" {
        return current_branch();
    }

    // Get the effective source (handles develop -> main fallback)
    let effective = workflow.effective_source(branch_type);

    // Check if it's a pattern (e.g., "release/*")
    if effective.contains('*') {
        // For patterns, the user must specify at runtime via --from
        bail!(
            "Branch type '{}' uses a pattern source '{}'. Use --from to specify the source branch.",
            branch_type.name,
            effective
        );
    }

    // Verify the branch exists
    if !branch_exists(effective)? {
        bail!(
            "Source branch '{}' does not exist. Create it first or use --from to specify an alternative.",
            effective
        );
    }

    Ok(effective.to_string())
}

/// Verify workflow rules before an operation.
pub fn verify_rules(workflow: &Workflow, operation: &str) -> Result<()> {
    if let Some(ref rules) = workflow.rules {
        if rules.require_clean_tree == Some(true) {
            require_clean_tree(operation)?;
        }

        if rules.require_up_to_date == Some(true) {
            // Fetch and check if we're up to date
            // This is advisory - we don't fail, just warn
            let _ = git_output(&["fetch", "--quiet"]);
        }
    }
    Ok(())
}

/// Format a merge strategy for display.
#[allow(dead_code)]
pub fn format_merge_strategy(strategy: &MergeStrategy) -> &'static str {
    match strategy {
        MergeStrategy::Merge => "merge",
        MergeStrategy::Squash => "squash",
        MergeStrategy::Rebase => "rebase",
        MergeStrategy::FfOnly => "fast-forward",
        MergeStrategy::CherryPick => "cherry-pick",
    }
}

/// Check if we're in a git repository.
pub fn in_git_repo() -> bool {
    git_output(&["rev-parse", "--git-dir"]).is_ok()
}

/// Get the repository root directory.
pub fn repo_root() -> Result<String> {
    git_output(&["rev-parse", "--show-toplevel"])
        .context("Not in a git repository")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::workflow::BranchTarget;

    #[test]
    fn test_make_branch_name() {
        let branch_type = BranchType {
            name: "feature".to_string(),
            prefix: "feature/".to_string(),
            ..Default::default()
        };
        assert_eq!(make_branch_name(&branch_type, "login"), "feature/login");

        let no_prefix = BranchType {
            name: "feature".to_string(),
            prefix: "".to_string(),
            ..Default::default()
        };
        assert_eq!(make_branch_name(&no_prefix, "login"), "login");
    }

    #[test]
    fn test_extract_branch_name() {
        let branch_type = BranchType {
            name: "feature".to_string(),
            prefix: "feature/".to_string(),
            ..Default::default()
        };
        assert_eq!(extract_branch_name(&branch_type, "feature/login"), "login");
        assert_eq!(extract_branch_name(&branch_type, "login"), "login");
    }
}
