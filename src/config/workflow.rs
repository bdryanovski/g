//! Workflow configuration types for customizable git branching strategies.
//!
//! This module defines the configuration schema for `g workflow`, enabling
//! users to define custom branching models similar to git-flow but with
//! full flexibility over naming conventions, merge strategies, and lifecycle
//! rules.
//!
//! ## Configuration hierarchy
//!
//! Workflows are loaded in the following order (later sources override earlier):
//! 1. Built-in presets (gitflow, github-flow, trunk-based, etc.)
//! 2. Global config (`~/.config/g/config.toml`)
//! 3. Repo-local config (`.g/workflow.toml`)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Top-level workflow configuration ─────────────────────────────────────────

/// Root workflow configuration containing all defined workflows.
///
/// This is stored under `[workflows]` in the config file.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct WorkflowsConfig {
    /// Name of the active workflow (e.g., "gitflow", "github-flow", "my-custom").
    /// When `None`, the first available workflow is used.
    #[serde(default)]
    pub default: Option<String>,

    /// Named workflow definitions.
    /// Each key is a workflow name, value is the full workflow configuration.
    #[serde(flatten)]
    pub workflows: HashMap<String, Workflow>,
}

/// A complete workflow definition with branch types, rules, and hooks.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Workflow {
    /// Main/production branch name (e.g., "main", "master").
    #[serde(default = "default_main_branch")]
    pub main_branch: String,

    /// Development/integration branch name (e.g., "develop").
    /// When `None`, features merge directly to main.
    #[serde(default)]
    pub develop_branch: Option<String>,

    /// Supported LTS version branches for multi-version workflows (e.g., ["v1.x", "v2.x"]).
    #[serde(default)]
    pub supported_versions: Option<Vec<String>>,

    /// Regex pattern for ticket IDs in branch names (e.g., "[A-Z]+-\\d+" for JIRA).
    #[serde(default)]
    pub ticket_pattern: Option<String>,

    /// Branch type definitions (feature, hotfix, release, etc.).
    #[serde(default)]
    pub types: Vec<BranchType>,

    /// Lifecycle hooks that run at workflow events.
    #[serde(default)]
    pub hooks: Option<WorkflowHooks>,

    /// Global rules that apply to all branch types.
    #[serde(default)]
    pub rules: Option<WorkflowRules>,
}

fn default_main_branch() -> String {
    "main".to_string()
}

impl Default for Workflow {
    fn default() -> Self {
        Self {
            main_branch: default_main_branch(),
            develop_branch: None,
            supported_versions: None,
            ticket_pattern: None,
            types: Vec::new(),
            hooks: None,
            rules: None,
        }
    }
}

// ─── Branch type configuration ────────────────────────────────────────────────

/// A branch type definition (e.g., feature, hotfix, release).
///
/// Each type specifies naming conventions, source/target branches,
/// merge strategy, and lifecycle options.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BranchType {
    /// Branch type name (e.g., "feature", "hotfix", "release").
    pub name: String,

    /// Branch name prefix (e.g., "feature/", "hotfix/").
    /// Empty string means no prefix.
    #[serde(default)]
    pub prefix: String,

    /// Source branch to create from.
    /// Can be: "main", "develop", "HEAD", or a pattern like "release/*".
    #[serde(default = "default_source")]
    pub source: String,

    /// Target branch(es) to merge into.
    /// Can be: a single branch, multiple branches, or null for experimental.
    #[serde(default)]
    pub target: Option<BranchTarget>,

    /// Merge strategy when finishing the branch.
    #[serde(default)]
    pub merge_strategy: MergeStrategy,

    // ─── Lifecycle options ────────────────────────────────────────────────────
    /// Delete branch after successful merge.
    #[serde(default)]
    pub delete_after_merge: Option<bool>,

    /// Require a pull request (no direct merge allowed).
    #[serde(default)]
    pub require_pr: Option<bool>,

    /// Mark as ephemeral (experimental, may be abandoned).
    #[serde(default)]
    pub ephemeral: Option<bool>,

    /// Auto-cleanup branches older than N days (for ephemeral branches).
    #[serde(default)]
    pub auto_cleanup_days: Option<u32>,

    /// Warn if branch age exceeds N hours (for trunk-based).
    #[serde(default)]
    pub max_age_hours: Option<u32>,

    // ─── Tagging options ──────────────────────────────────────────────────────
    /// Create a git tag when finishing the branch.
    #[serde(default)]
    pub tag_on_finish: Option<bool>,

    /// Tag pattern with variables: {name}, {version}, {date}, {type}.
    /// Example: "v{version}", "release-{name}-{date}".
    #[serde(default)]
    pub tag_pattern: Option<String>,

    // ─── Validation options ───────────────────────────────────────────────────
    /// Regex pattern for validating the branch name portion (after prefix).
    /// Example: "^[a-z0-9-]+$" for lowercase alphanumeric with hyphens.
    #[serde(default)]
    pub naming_pattern: Option<String>,

    /// Require a ticket ID in the branch name.
    #[serde(default)]
    pub require_ticket: Option<bool>,

    // ─── PR integration options ───────────────────────────────────────────────
    /// PR template name to use when creating pull requests.
    #[serde(default)]
    pub pr_template: Option<String>,

    /// Default labels to apply when creating pull requests.
    #[serde(default)]
    pub pr_labels: Option<Vec<String>>,

    /// Default reviewers to request when creating pull requests.
    #[serde(default)]
    pub pr_reviewers: Option<Vec<String>>,

    /// Create PRs as drafts by default.
    #[serde(default)]
    pub pr_draft: Option<bool>,
}

fn default_source() -> String {
    "main".to_string()
}

impl Default for BranchType {
    fn default() -> Self {
        Self {
            name: String::new(),
            prefix: String::new(),
            source: default_source(),
            target: None,
            merge_strategy: MergeStrategy::default(),
            delete_after_merge: None,
            require_pr: None,
            ephemeral: None,
            auto_cleanup_days: None,
            max_age_hours: None,
            tag_on_finish: None,
            tag_pattern: None,
            naming_pattern: None,
            require_ticket: None,
            pr_template: None,
            pr_labels: None,
            pr_reviewers: None,
            pr_draft: None,
        }
    }
}

// ─── Merge strategy ───────────────────────────────────────────────────────────

/// Merge strategy used when finishing a branch.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MergeStrategy {
    /// Regular merge commit (preserves history).
    #[default]
    Merge,
    /// Squash all commits into one (clean history).
    Squash,
    /// Rebase commits on top of target (linear history).
    Rebase,
    /// Fast-forward only (fails if not possible).
    FfOnly,
    /// Cherry-pick commits (for backports).
    CherryPick,
}

impl MergeStrategy {
    /// Returns the git arguments for this merge strategy.
    #[allow(dead_code)]
    pub fn git_args(&self) -> &[&str] {
        match self {
            MergeStrategy::Merge => &["--no-ff"],
            MergeStrategy::Squash => &["--squash"],
            MergeStrategy::Rebase => &[], // Handled separately
            MergeStrategy::FfOnly => &["--ff-only"],
            MergeStrategy::CherryPick => &[], // Handled separately
        }
    }

    /// Returns a human-readable description.
    #[allow(dead_code)]
    pub fn description(&self) -> &str {
        match self {
            MergeStrategy::Merge => "merge commit (preserves history)",
            MergeStrategy::Squash => "squash (single commit)",
            MergeStrategy::Rebase => "rebase (linear history)",
            MergeStrategy::FfOnly => "fast-forward only",
            MergeStrategy::CherryPick => "cherry-pick (for backports)",
        }
    }
}

impl std::fmt::Display for MergeStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MergeStrategy::Merge => write!(f, "merge"),
            MergeStrategy::Squash => write!(f, "squash"),
            MergeStrategy::Rebase => write!(f, "rebase"),
            MergeStrategy::FfOnly => write!(f, "ff-only"),
            MergeStrategy::CherryPick => write!(f, "cherry-pick"),
        }
    }
}

// ─── Branch target ────────────────────────────────────────────────────────────

/// Target branch(es) for merging.
///
/// Can be a single branch name or multiple branches (for hotfixes that
/// need to go to both main and develop).
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum BranchTarget {
    /// Single target branch (e.g., "main").
    Single(String),
    /// Multiple target branches (e.g., ["main", "develop"]).
    Multiple(Vec<String>),
}

impl BranchTarget {
    /// Returns target branches as a slice.
    pub fn as_slice(&self) -> Vec<&str> {
        match self {
            BranchTarget::Single(s) => vec![s.as_str()],
            BranchTarget::Multiple(v) => v.iter().map(|s| s.as_str()).collect(),
        }
    }

    /// Returns the primary (first) target branch.
    pub fn primary(&self) -> &str {
        match self {
            BranchTarget::Single(s) => s,
            BranchTarget::Multiple(v) => v.first().map(|s| s.as_str()).unwrap_or("main"),
        }
    }
}

impl Default for BranchTarget {
    fn default() -> Self {
        BranchTarget::Single("main".to_string())
    }
}

impl std::fmt::Display for BranchTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BranchTarget::Single(s) => write!(f, "{}", s),
            BranchTarget::Multiple(v) => write!(f, "{}", v.join(" + ")),
        }
    }
}

// ─── Workflow hooks ───────────────────────────────────────────────────────────

/// Lifecycle hooks that run at workflow events.
///
/// Each hook is a list of shell commands executed in order.
/// If any command exits non-zero, the workflow operation is aborted.
///
/// Available environment variables in hooks:
/// - `$G_WORKFLOW` - Active workflow name
/// - `$G_BRANCH_TYPE` - Branch type (feature, hotfix, etc.)
/// - `$G_BRANCH_NAME` - Full branch name
/// - `$G_SOURCE` - Source branch
/// - `$G_TARGET` - Target branch(es)
/// - `$G_TICKET` - Ticket ID (if ticket-linked)
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct WorkflowHooks {
    /// Commands to run BEFORE creating a new branch.
    #[serde(default)]
    pub pre_start: Option<Vec<String>>,

    /// Commands to run AFTER creating a new branch.
    #[serde(default)]
    pub post_start: Option<Vec<String>>,

    /// Commands to run BEFORE merging (e.g., tests, linting).
    #[serde(default)]
    pub pre_finish: Option<Vec<String>>,

    /// Commands to run AFTER successful merge.
    #[serde(default)]
    pub post_finish: Option<Vec<String>>,

    /// Commands to run when publishing (push + PR creation).
    #[serde(default)]
    pub on_publish: Option<Vec<String>>,
}

// ─── Workflow rules ───────────────────────────────────────────────────────────

/// Global rules that apply to all branch types in a workflow.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct WorkflowRules {
    /// Require a clean working tree before operations.
    #[serde(default)]
    pub require_clean_tree: Option<bool>,

    /// Require source branch to be up-to-date with remote.
    #[serde(default)]
    pub require_up_to_date: Option<bool>,

    /// Global regex pattern for branch names.
    #[serde(default)]
    pub branch_name_pattern: Option<String>,

    /// Warn about branches older than N days.
    #[serde(default)]
    pub max_branch_age_days: Option<u32>,

    /// Advisory reminder to use feature flags (trunk-based).
    #[serde(default)]
    pub require_feature_flags: Option<bool>,
}

// ─── Workflow documentation ───────────────────────────────────────────────────

/// Documentation for a workflow, including ASCII diagram and pros/cons.
///
/// Used for `g workflow info` and the interactive wizard.
#[derive(Debug, Clone)]
pub struct WorkflowDocs {
    /// Workflow name.
    pub name: &'static str,
    /// ASCII diagram showing the branching model.
    pub diagram: &'static str,
    /// Short description of the workflow.
    pub description: &'static str,
    /// Use cases where this workflow excels.
    pub use_cases: &'static [&'static str],
    /// Advantages of this workflow.
    pub pros: &'static [&'static str],
    /// Disadvantages of this workflow.
    pub cons: &'static [&'static str],
    /// Branch types included in this workflow.
    pub branch_types: &'static [&'static str],
}

// ─── Helper methods ───────────────────────────────────────────────────────────

impl Workflow {
    /// Find a branch type by name.
    pub fn get_type(&self, name: &str) -> Option<&BranchType> {
        self.types.iter().find(|t| t.name == name)
    }

    /// Find a branch type by matching a branch name against prefixes.
    ///
    /// Matching rules:
    /// 1. First, try to match branch types with non-empty prefixes
    /// 2. If no match, fall back to a branch type with empty prefix (catch-all)
    /// 3. Skip main/develop branches - they're not feature branches
    pub fn type_for_branch(&self, branch: &str) -> Option<&BranchType> {
        // Don't match the main or develop branches
        if branch == self.main_branch || self.develop_branch.as_deref() == Some(branch) {
            return None;
        }

        // First try to find a type with a matching prefix
        if let Some(t) = self
            .types
            .iter()
            .find(|t| !t.prefix.is_empty() && branch.starts_with(&t.prefix))
        {
            return Some(t);
        }

        // Fall back to a type with empty prefix (catch-all, e.g., github-flow's feature type)
        self.types.iter().find(|t| t.prefix.is_empty())
    }

    /// Get the effective source branch for a branch type.
    pub fn effective_source<'a>(&'a self, branch_type: &'a BranchType) -> &'a str {
        let source = &branch_type.source;
        if source == "develop" {
            self.develop_branch.as_deref().unwrap_or(&self.main_branch)
        } else if source == "main" {
            &self.main_branch
        } else {
            source
        }
    }

    /// Get the effective target branch(es) for a branch type.
    pub fn effective_target(&self, branch_type: &BranchType) -> Option<BranchTarget> {
        branch_type.target.clone().or_else(|| {
            // Default: merge back to source
            let source = self.effective_source(branch_type);
            Some(BranchTarget::Single(source.to_string()))
        })
    }

    /// Check if a branch name is valid for a given branch type.
    pub fn validate_branch_name(&self, branch_type: &BranchType, name: &str) -> Result<(), String> {
        // Check global rules first
        if let Some(ref rules) = self.rules {
            if let Some(ref pattern) = rules.branch_name_pattern {
                let re = regex::Regex::new(pattern)
                    .map_err(|e| format!("Invalid branch name pattern: {}", e))?;
                if !re.is_match(name) {
                    return Err(format!(
                        "Branch name '{}' does not match pattern '{}'",
                        name, pattern
                    ));
                }
            }
        }

        // Check type-specific pattern
        if let Some(ref pattern) = branch_type.naming_pattern {
            let re =
                regex::Regex::new(pattern).map_err(|e| format!("Invalid naming pattern: {}", e))?;
            if !re.is_match(name) {
                return Err(format!(
                    "Branch name '{}' does not match pattern '{}' for type '{}'",
                    name, pattern, branch_type.name
                ));
            }
        }

        // Check ticket requirement
        if branch_type.require_ticket == Some(true) {
            if let Some(ref ticket_pattern) = self.ticket_pattern {
                let re = regex::Regex::new(ticket_pattern)
                    .map_err(|e| format!("Invalid ticket pattern: {}", e))?;
                if !re.is_match(name) {
                    return Err(format!(
                        "Branch name '{}' must include a ticket ID matching pattern '{}'",
                        name, ticket_pattern
                    ));
                }
            } else {
                return Err(
                    "Ticket is required but no ticket_pattern is defined in workflow".to_string(),
                );
            }
        }

        Ok(())
    }
}

impl WorkflowsConfig {
    /// Get the active workflow.
    pub fn active(&self) -> Option<(&String, &Workflow)> {
        if let Some(ref name) = self.default {
            self.workflows.get(name).map(|w| (name, w))
        } else {
            self.workflows.iter().next()
        }
    }

    /// Get a workflow by name.
    pub fn get(&self, name: &str) -> Option<&Workflow> {
        self.workflows.get(name)
    }

    /// Check if any workflows are defined.
    pub fn is_empty(&self) -> bool {
        self.workflows.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_strategy_display() {
        assert_eq!(MergeStrategy::Squash.to_string(), "squash");
        assert_eq!(MergeStrategy::Merge.to_string(), "merge");
    }

    #[test]
    fn test_branch_target_as_slice() {
        let single = BranchTarget::Single("main".to_string());
        assert_eq!(single.as_slice(), vec!["main"]);

        let multiple = BranchTarget::Multiple(vec!["main".to_string(), "develop".to_string()]);
        assert_eq!(multiple.as_slice(), vec!["main", "develop"]);
    }

    #[test]
    fn test_workflow_type_lookup() {
        let workflow = Workflow {
            types: vec![
                BranchType {
                    name: "feature".to_string(),
                    prefix: "feature/".to_string(),
                    ..Default::default()
                },
                BranchType {
                    name: "hotfix".to_string(),
                    prefix: "hotfix/".to_string(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        assert!(workflow.get_type("feature").is_some());
        assert!(workflow.get_type("unknown").is_none());

        assert_eq!(
            workflow.type_for_branch("feature/login").unwrap().name,
            "feature"
        );
        assert_eq!(
            workflow.type_for_branch("hotfix/CVE-123").unwrap().name,
            "hotfix"
        );
        assert!(workflow.type_for_branch("random-branch").is_none());
    }

    #[test]
    fn test_deserialize_workflow() {
        let toml = r#"
            main_branch = "main"
            develop_branch = "develop"
            
            [[types]]
            name = "feature"
            prefix = "feature/"
            source = "develop"
            target = "develop"
            merge_strategy = "squash"
            delete_after_merge = true
        "#;

        let workflow: Workflow = toml::from_str(toml).unwrap();
        assert_eq!(workflow.main_branch, "main");
        assert_eq!(workflow.develop_branch, Some("develop".to_string()));
        assert_eq!(workflow.types.len(), 1);
        assert_eq!(workflow.types[0].name, "feature");
        assert_eq!(workflow.types[0].merge_strategy, MergeStrategy::Squash);
    }

    #[test]
    fn test_deserialize_multiple_targets() {
        let toml = r#"
            name = "hotfix"
            prefix = "hotfix/"
            source = "main"
            target = ["main", "develop"]
            merge_strategy = "merge"
        "#;

        let branch_type: BranchType = toml::from_str(toml).unwrap();
        match branch_type.target {
            Some(BranchTarget::Multiple(targets)) => {
                assert_eq!(targets, vec!["main", "develop"]);
            }
            _ => panic!("Expected multiple targets"),
        }
    }

    #[test]
    fn test_branch_name_validation_basic() {
        let workflow = Workflow {
            types: vec![BranchType {
                name: "feature".to_string(),
                prefix: "feature/".to_string(),
                naming_pattern: None,
                ..Default::default()
            }],
            ..Default::default()
        };

        // No pattern means any name is valid
        assert!(workflow
            .validate_branch_name(workflow.get_type("feature").unwrap(), "any-name")
            .is_ok());
    }

    #[test]
    fn test_branch_name_validation_with_pattern() {
        let workflow = Workflow {
            types: vec![BranchType {
                name: "feature".to_string(),
                prefix: "feature/".to_string(),
                naming_pattern: Some(r"^[a-z][a-z0-9-]+$".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let branch_type = workflow.get_type("feature").unwrap();

        // Valid names
        assert!(workflow
            .validate_branch_name(branch_type, "login-page")
            .is_ok());
        assert!(workflow
            .validate_branch_name(branch_type, "add-user-auth")
            .is_ok());

        // Invalid names
        assert!(workflow.validate_branch_name(branch_type, "Login").is_err());
        assert!(workflow
            .validate_branch_name(branch_type, "123-feature")
            .is_err());
        assert!(workflow
            .validate_branch_name(branch_type, "feature with spaces")
            .is_err());
    }

    #[test]
    fn test_workflows_config_active() {
        let mut config = WorkflowsConfig::default();
        config.workflows.insert(
            "gitflow".to_string(),
            Workflow {
                main_branch: "main".to_string(),
                ..Default::default()
            },
        );
        config.workflows.insert(
            "custom".to_string(),
            Workflow {
                main_branch: "production".to_string(),
                ..Default::default()
            },
        );

        // No default set - returns first (arbitrary)
        assert!(config.active().is_some());

        // Set default
        config.default = Some("custom".to_string());
        let (name, workflow) = config.active().unwrap();
        assert_eq!(name, "custom");
        assert_eq!(workflow.main_branch, "production");
    }

    #[test]
    fn test_effective_source_with_develop() {
        let workflow = Workflow {
            main_branch: "main".to_string(),
            develop_branch: Some("develop".to_string()),
            types: vec![BranchType {
                name: "feature".to_string(),
                source: "develop".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };

        let feature = workflow.get_type("feature").unwrap();
        assert_eq!(workflow.effective_source(feature), "develop");
    }

    #[test]
    fn test_effective_source_fallback_to_main() {
        let workflow = Workflow {
            main_branch: "main".to_string(),
            develop_branch: None, // No develop branch
            types: vec![BranchType {
                name: "feature".to_string(),
                source: "develop".to_string(), // References develop but it doesn't exist
                ..Default::default()
            }],
            ..Default::default()
        };

        let feature = workflow.get_type("feature").unwrap();
        // Should fall back to main since develop doesn't exist
        assert_eq!(workflow.effective_source(feature), "main");
    }

    #[test]
    fn test_branch_target_display() {
        let single = BranchTarget::Single("main".to_string());
        assert_eq!(single.to_string(), "main");

        let multiple = BranchTarget::Multiple(vec!["main".to_string(), "develop".to_string()]);
        assert_eq!(multiple.to_string(), "main + develop");
    }

    #[test]
    fn test_workflow_rules_defaults() {
        let rules = WorkflowRules::default();
        assert_eq!(rules.require_clean_tree, None);
        assert_eq!(rules.require_up_to_date, None);
        assert_eq!(rules.branch_name_pattern, None);
    }

    #[test]
    fn test_deserialize_workflow_with_hooks() {
        let toml = r#"
            main_branch = "main"
            
            [hooks]
            pre_start = ["npm run lint"]
            post_finish = ["notify-team.sh"]
        "#;

        let workflow: Workflow = toml::from_str(toml).unwrap();
        let hooks = workflow.hooks.unwrap();
        assert_eq!(hooks.pre_start, Some(vec!["npm run lint".to_string()]));
        assert_eq!(hooks.post_finish, Some(vec!["notify-team.sh".to_string()]));
    }

    #[test]
    fn test_deserialize_workflow_with_rules() {
        let toml = r#"
            main_branch = "main"
            
            [rules]
            require_clean_tree = true
            require_up_to_date = true
            max_branch_age_days = 7
        "#;

        let workflow: Workflow = toml::from_str(toml).unwrap();
        let rules = workflow.rules.unwrap();
        assert_eq!(rules.require_clean_tree, Some(true));
        assert_eq!(rules.require_up_to_date, Some(true));
        assert_eq!(rules.max_branch_age_days, Some(7));
    }

    #[test]
    fn test_branch_type_with_ticket_requirement() {
        let toml = r#"
            name = "fix"
            prefix = "fix/"
            source = "main"
            merge_strategy = "squash"
            require_ticket = true
        "#;

        let branch_type: BranchType = toml::from_str(toml).unwrap();
        assert_eq!(branch_type.require_ticket, Some(true));
    }

    #[test]
    fn test_branch_type_with_tag_on_finish() {
        let toml = r#"
            name = "release"
            prefix = "release/"
            source = "develop"
            target = "main"
            merge_strategy = "merge"
            tag_on_finish = true
            tag_pattern = "v{version}"
        "#;

        let branch_type: BranchType = toml::from_str(toml).unwrap();
        assert_eq!(branch_type.tag_on_finish, Some(true));
        assert_eq!(branch_type.tag_pattern, Some("v{version}".to_string()));
    }

    #[test]
    fn test_all_merge_strategies() {
        let strategies = [
            ("merge", MergeStrategy::Merge),
            ("squash", MergeStrategy::Squash),
            ("rebase", MergeStrategy::Rebase),
            ("ff-only", MergeStrategy::FfOnly),
            ("cherry-pick", MergeStrategy::CherryPick),
        ];

        for (name, expected) in strategies {
            let toml = format!(
                r#"
                name = "test"
                prefix = "test/"
                source = "main"
                merge_strategy = "{}"
            "#,
                name
            );
            let bt: BranchType = toml::from_str(&toml).unwrap();
            assert_eq!(bt.merge_strategy, expected, "Failed for {}", name);
        }
    }

    #[test]
    fn test_workflows_config_get() {
        let mut config = WorkflowsConfig::default();
        config
            .workflows
            .insert("gitflow".to_string(), Workflow::default());

        assert!(config.get("gitflow").is_some());
        assert!(config.get("nonexistent").is_none());
    }

    #[test]
    fn test_workflows_config_is_empty() {
        let config = WorkflowsConfig::default();
        assert!(config.is_empty());

        let mut config2 = WorkflowsConfig::default();
        config2
            .workflows
            .insert("test".to_string(), Workflow::default());
        assert!(!config2.is_empty());
    }

    #[test]
    fn test_type_for_branch_with_prefix() {
        let workflow = Workflow {
            main_branch: "main".to_string(),
            develop_branch: Some("develop".to_string()),
            types: vec![
                BranchType {
                    name: "feature".to_string(),
                    prefix: "feature/".to_string(),
                    ..Default::default()
                },
                BranchType {
                    name: "hotfix".to_string(),
                    prefix: "hotfix/".to_string(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        // Should match prefixed branches
        assert_eq!(
            workflow.type_for_branch("feature/login").unwrap().name,
            "feature"
        );
        assert_eq!(
            workflow.type_for_branch("hotfix/CVE-123").unwrap().name,
            "hotfix"
        );

        // Should not match unrecognized branches (no catch-all)
        assert!(workflow.type_for_branch("random-branch").is_none());

        // Should not match main/develop
        assert!(workflow.type_for_branch("main").is_none());
        assert!(workflow.type_for_branch("develop").is_none());
    }

    #[test]
    fn test_type_for_branch_empty_prefix_catchall() {
        // github-flow style: empty prefix catches all non-main branches
        let workflow = Workflow {
            main_branch: "main".to_string(),
            develop_branch: None,
            types: vec![BranchType {
                name: "feature".to_string(),
                prefix: "".to_string(), // Empty prefix = catch-all
                ..Default::default()
            }],
            ..Default::default()
        };

        // Should match any branch name
        assert_eq!(
            workflow.type_for_branch("my-feature").unwrap().name,
            "feature"
        );
        assert_eq!(
            workflow.type_for_branch("experiment-f").unwrap().name,
            "feature"
        );
        assert_eq!(
            workflow.type_for_branch("fix-bug-123").unwrap().name,
            "feature"
        );
        assert_eq!(
            workflow.type_for_branch("anything-goes").unwrap().name,
            "feature"
        );

        // Should NOT match main branch
        assert!(workflow.type_for_branch("main").is_none());
    }

    #[test]
    fn test_type_for_branch_prefix_takes_priority() {
        // Mixed workflow: specific prefixes + catch-all
        let workflow = Workflow {
            main_branch: "main".to_string(),
            develop_branch: None,
            types: vec![
                BranchType {
                    name: "hotfix".to_string(),
                    prefix: "hotfix/".to_string(),
                    ..Default::default()
                },
                BranchType {
                    name: "feature".to_string(),
                    prefix: "".to_string(), // Catch-all
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        // Specific prefix should take priority
        assert_eq!(
            workflow.type_for_branch("hotfix/urgent").unwrap().name,
            "hotfix"
        );

        // Other branches fall through to catch-all
        assert_eq!(
            workflow.type_for_branch("my-feature").unwrap().name,
            "feature"
        );
        assert_eq!(workflow.type_for_branch("random").unwrap().name, "feature");

        // Main is still excluded
        assert!(workflow.type_for_branch("main").is_none());
    }

    #[test]
    fn test_type_for_branch_excludes_develop() {
        let workflow = Workflow {
            main_branch: "main".to_string(),
            develop_branch: Some("develop".to_string()),
            types: vec![BranchType {
                name: "feature".to_string(),
                prefix: "".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };

        // Should not match develop branch even with catch-all
        assert!(workflow.type_for_branch("develop").is_none());

        // But should match other branches
        assert_eq!(
            workflow.type_for_branch("my-branch").unwrap().name,
            "feature"
        );
    }
}
