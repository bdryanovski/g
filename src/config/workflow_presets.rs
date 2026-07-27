//! Built-in workflow presets with embedded documentation.
//!
//! This module provides ready-to-use workflow configurations for common
//! branching strategies. Each preset includes:
//! - Complete workflow configuration
//! - ASCII diagram showing the branching model
//! - Use cases, pros, and cons for decision-making
//!
//! ## Available presets
//!
//! - `gitflow` - Classic Git Flow with develop/release/hotfix
//! - `github-flow` - Simple feature branches to main
//! - `trunk-based` - Very short-lived branches, frequent integration
//! - `release-train` - Scheduled release cadence
//! - `experiment` - Spike/prototype branches with auto-cleanup
//! - `ticket-linked` - Branches require issue tracker IDs
//! - `multi-version` - LTS support with backport workflow

use super::workflow::{
    BranchTarget, BranchType, MergeStrategy, Workflow, WorkflowDocs, WorkflowRules,
    WorkflowsConfig,
};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════════
// WORKFLOW DOCUMENTATION
// ═══════════════════════════════════════════════════════════════════════════════

/// All workflow documentation entries.
pub const WORKFLOW_DOCS: &[WorkflowDocs] = &[
    // ─── Git Flow ──────────────────────────────────────────────────────────────
    WorkflowDocs {
        name: "gitflow",
        diagram: r#"
main ──────────────────────●────────────────●──────
                          ↑                ↑
release/* ────────────●───┘    ────────●───┘
                     ↑                ↑
develop ─────●───●───●───●───●───●───●───●───●─────
            ↑       ↑       ↑
feature/* ──┴───────┴───────┘
hotfix/*  ──────────────────────────●→main+develop
"#,
        description: "Classic branching model with parallel development and release preparation.",
        use_cases: &[
            "Scheduled releases (weekly, monthly)",
            "Multiple versions in production",
            "Teams with QA gates before release",
            "Enterprise software with release cycles",
        ],
        pros: &[
            "Clear separation of concerns",
            "Parallel release preparation",
            "Well-documented, widely understood",
            "Supports multiple release versions",
        ],
        cons: &[
            "Complex branch management",
            "Slower for CI/CD pipelines",
            "Merge conflicts between long-lived branches",
            "Overhead for small teams/projects",
        ],
        branch_types: &["feature", "release", "hotfix"],
    },
    // ─── GitHub Flow ───────────────────────────────────────────────────────────
    WorkflowDocs {
        name: "github-flow",
        diagram: r#"
main ──────●──────●──────●──────●──────●──────
          ↑      ↑      ↑      ↑      ↑
feature ──┴──────┴──────┴──────┴──────┘
"#,
        description: "Simple feature branch workflow with PRs directly to main.",
        use_cases: &[
            "Continuous deployment",
            "Web applications",
            "Small to medium teams",
            "Projects with strong CI/CD",
        ],
        pros: &[
            "Simple to understand and use",
            "Fast iteration cycles",
            "PR-centric review process",
            "Works well with CI/CD",
        ],
        cons: &[
            "No staging/release separation",
            "Requires robust testing before merge",
            "No parallel release preparation",
            "Harder to support multiple versions",
        ],
        branch_types: &["feature"],
    },
    // ─── Trunk-Based ───────────────────────────────────────────────────────────
    WorkflowDocs {
        name: "trunk-based",
        diagram: r#"
main ──●──●──●──●──●──●──●──●──●──●──●──●──●──
       ↑  ↑  ↑  ↑
       └──┴──┴──┘ (very short branches, <1 day)
"#,
        description: "Developers commit to main frequently via very short-lived branches.",
        use_cases: &[
            "High-performing engineering teams",
            "Continuous deployment (multiple per day)",
            "Teams with strong automated testing",
            "Feature flag infrastructure",
        ],
        pros: &[
            "Minimal merge conflicts",
            "Fast feedback loops",
            "Always deployable main branch",
            "Encourages small, incremental changes",
        ],
        cons: &[
            "Requires strong CI pipeline",
            "Feature flags add complexity",
            "Not suitable for less experienced teams",
            "Harder to manage large features",
        ],
        branch_types: &["feature"],
    },
    // ─── Release Train ─────────────────────────────────────────────────────────
    WorkflowDocs {
        name: "release-train",
        diagram: r#"
main ──────────────────────────────────────────
         ↓           ↓           ↓
    release/2024.01  2024.02     2024.03
         ↑           ↑           ↑
feature ─┴───────────┴───────────┘

cherry-pick: main → release/* (for urgent fixes)
"#,
        description: "Scheduled release cadence with time-boxed release branches.",
        use_cases: &[
            "Mobile apps with app store review cycles",
            "Enterprise software with fixed schedules",
            "Teams coordinating across time zones",
            "Products with marketing-driven releases",
        ],
        pros: &[
            "Predictable release schedule",
            "Parallel feature development",
            "Clear cutoff dates for features",
            "Easy to plan QA cycles",
        ],
        cons: &[
            "Cherry-pick overhead for fixes",
            "Features may miss the train",
            "Requires discipline on cutoff dates",
            "Can accumulate technical debt between trains",
        ],
        branch_types: &["feature", "release", "cherry-pick"],
    },
    // ─── Experiment ────────────────────────────────────────────────────────────
    WorkflowDocs {
        name: "experiment",
        diagram: r#"
main ─────────────────────────────────────────
  ↑
feature/login ────●────●────●───→ (merge to main)

exp/new-db ────●────●────● (may be abandoned)
spike/perf ────●────● (throwaway, auto-deleted)
"#,
        description: "Supports experimental and spike branches with auto-cleanup.",
        use_cases: &[
            "R&D and prototyping",
            "Architectural spikes",
            "Performance investigations",
            "Trying new technologies",
        ],
        pros: &[
            "Low-commitment exploration",
            "Auto-cleanup prevents branch clutter",
            "Clear distinction: production vs experiment",
            "Encourages innovation",
        ],
        cons: &[
            "Risk of abandoned work",
            "May duplicate effort across experiments",
            "Need discipline to graduate experiments",
            "Can distract from main work",
        ],
        branch_types: &["feature", "experiment", "spike"],
    },
    // ─── Ticket-Linked ─────────────────────────────────────────────────────────
    WorkflowDocs {
        name: "ticket-linked",
        diagram: r#"
main ─────────────────────────────────────────
  ↑           ↑           ↑
feature/PROJ-123-login    │           │
              fix/PROJ-456-null-ptr   │
                          hotfix/PROJ-789-security

Branch names MUST include ticket ID (validated)
"#,
        description: "Branch names must include issue tracker IDs for traceability.",
        use_cases: &[
            "Teams using JIRA, Linear, GitHub Issues",
            "Audit and compliance requirements",
            "Automatic PR <-> issue linking",
            "Sprint/iteration tracking",
        ],
        pros: &[
            "Full traceability from code to ticket",
            "Automatic PR descriptions from tickets",
            "Easy to find related branches",
            "Supports audit requirements",
        ],
        cons: &[
            "Verbose branch names",
            "Requires ticket before branching",
            "Friction for quick fixes",
            "Depends on external system",
        ],
        branch_types: &["feature", "fix", "hotfix"],
    },
    // ─── Multi-Version ─────────────────────────────────────────────────────────
    WorkflowDocs {
        name: "multi-version",
        diagram: r#"
main (v3) ──────────────────────────────────────
v2.x ───────────────────────────────────────────
v1.x ───────────────────────────────────────────

hotfix/CVE-2024:
  main ←── fix ──→ cherry-pick to v2.x, v1.x

backport/v2-feature:
  main ←── feature ──→ cherry-pick to v2.x
"#,
        description: "Supports multiple maintained versions with backport workflow.",
        use_cases: &[
            "Libraries and SDKs",
            "Long-term support (LTS) products",
            "Enterprise customers on older versions",
            "Security patches across versions",
        ],
        pros: &[
            "Security fixes reach all users",
            "Clear version maintenance policy",
            "Supports enterprise customers",
            "Structured backport process",
        ],
        cons: &[
            "Cherry-pick management overhead",
            "Potential for divergence between versions",
            "More branches to maintain",
            "Testing across multiple versions",
        ],
        branch_types: &["feature", "hotfix", "backport"],
    },
];

/// Get documentation for a specific workflow.
pub fn get_docs(name: &str) -> Option<&'static WorkflowDocs> {
    WORKFLOW_DOCS.iter().find(|d| d.name == name)
}

/// Get all available preset names.
pub fn preset_names() -> Vec<&'static str> {
    WORKFLOW_DOCS.iter().map(|d| d.name).collect()
}

// ═══════════════════════════════════════════════════════════════════════════════
// WORKFLOW CONFIGURATIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Get a preset workflow configuration by name.
pub fn get_preset(name: &str) -> Option<Workflow> {
    match name {
        "gitflow" => Some(gitflow()),
        "github-flow" => Some(github_flow()),
        "trunk-based" => Some(trunk_based()),
        "release-train" => Some(release_train()),
        "experiment" => Some(experiment()),
        "ticket-linked" => Some(ticket_linked()),
        "multi-version" => Some(multi_version()),
        _ => None,
    }
}

/// Get all presets as a WorkflowsConfig.
pub fn all_presets() -> WorkflowsConfig {
    let mut workflows = HashMap::new();
    for name in preset_names() {
        if let Some(workflow) = get_preset(name) {
            workflows.insert(name.to_string(), workflow);
        }
    }
    WorkflowsConfig {
        default: Some("github-flow".to_string()),
        workflows,
    }
}

// ─── Git Flow ──────────────────────────────────────────────────────────────────

fn gitflow() -> Workflow {
    Workflow {
        main_branch: "main".to_string(),
        develop_branch: Some("develop".to_string()),
        supported_versions: None,
        ticket_pattern: None,
        types: vec![
            BranchType {
                name: "feature".to_string(),
                prefix: "feature/".to_string(),
                source: "develop".to_string(),
                target: Some(BranchTarget::Single("develop".to_string())),
                merge_strategy: MergeStrategy::Squash,
                delete_after_merge: Some(true),
                require_pr: Some(true),
                ..Default::default()
            },
            BranchType {
                name: "release".to_string(),
                prefix: "release/".to_string(),
                source: "develop".to_string(),
                target: Some(BranchTarget::Multiple(vec![
                    "main".to_string(),
                    "develop".to_string(),
                ])),
                merge_strategy: MergeStrategy::Merge,
                delete_after_merge: Some(true),
                tag_on_finish: Some(true),
                tag_pattern: Some("v{version}".to_string()),
                ..Default::default()
            },
            BranchType {
                name: "hotfix".to_string(),
                prefix: "hotfix/".to_string(),
                source: "main".to_string(),
                target: Some(BranchTarget::Multiple(vec![
                    "main".to_string(),
                    "develop".to_string(),
                ])),
                merge_strategy: MergeStrategy::Merge,
                delete_after_merge: Some(true),
                tag_on_finish: Some(true),
                ..Default::default()
            },
        ],
        hooks: None,
        rules: Some(WorkflowRules {
            require_clean_tree: Some(true),
            require_up_to_date: Some(true),
            ..Default::default()
        }),
    }
}

// ─── GitHub Flow ───────────────────────────────────────────────────────────────

fn github_flow() -> Workflow {
    Workflow {
        main_branch: "main".to_string(),
        develop_branch: None,
        supported_versions: None,
        ticket_pattern: None,
        types: vec![BranchType {
            name: "feature".to_string(),
            prefix: "".to_string(), // No prefix for simplicity
            source: "main".to_string(),
            target: Some(BranchTarget::Single("main".to_string())),
            merge_strategy: MergeStrategy::Squash,
            delete_after_merge: Some(true),
            require_pr: Some(true),
            ..Default::default()
        }],
        hooks: None,
        rules: Some(WorkflowRules {
            require_clean_tree: Some(true),
            ..Default::default()
        }),
    }
}

// ─── Trunk-Based ───────────────────────────────────────────────────────────────

fn trunk_based() -> Workflow {
    Workflow {
        main_branch: "main".to_string(),
        develop_branch: None,
        supported_versions: None,
        ticket_pattern: None,
        types: vec![BranchType {
            name: "feature".to_string(),
            prefix: "".to_string(),
            source: "main".to_string(),
            target: Some(BranchTarget::Single("main".to_string())),
            merge_strategy: MergeStrategy::Rebase,
            delete_after_merge: Some(true),
            require_pr: Some(false), // Direct push OK for small changes
            max_age_hours: Some(24), // Warn if branch lives > 1 day
            ..Default::default()
        }],
        hooks: None,
        rules: Some(WorkflowRules {
            require_clean_tree: Some(true),
            require_up_to_date: Some(true),
            require_feature_flags: Some(true), // Advisory
            ..Default::default()
        }),
    }
}

// ─── Release Train ─────────────────────────────────────────────────────────────

fn release_train() -> Workflow {
    Workflow {
        main_branch: "main".to_string(),
        develop_branch: None,
        supported_versions: None,
        ticket_pattern: None,
        types: vec![
            BranchType {
                name: "feature".to_string(),
                prefix: "feature/".to_string(),
                source: "main".to_string(),
                target: Some(BranchTarget::Single("main".to_string())),
                merge_strategy: MergeStrategy::Squash,
                delete_after_merge: Some(true),
                require_pr: Some(true),
                ..Default::default()
            },
            BranchType {
                name: "release".to_string(),
                prefix: "release/".to_string(),
                source: "main".to_string(),
                target: Some(BranchTarget::Single("main".to_string())),
                merge_strategy: MergeStrategy::Merge,
                delete_after_merge: Some(false), // Keep release branches
                tag_on_finish: Some(true),
                tag_pattern: Some("{name}".to_string()), // e.g., 2024.07
                naming_pattern: Some(r"\d{4}\.\d{2}".to_string()), // YYYY.MM
                ..Default::default()
            },
            BranchType {
                name: "cherry-pick".to_string(),
                prefix: "cherry/".to_string(),
                source: "main".to_string(), // Typically from main
                target: None,               // User picks release branch at runtime
                merge_strategy: MergeStrategy::CherryPick,
                delete_after_merge: Some(true),
                ..Default::default()
            },
        ],
        hooks: None,
        rules: Some(WorkflowRules {
            require_clean_tree: Some(true),
            require_up_to_date: Some(true),
            ..Default::default()
        }),
    }
}

// ─── Experiment ────────────────────────────────────────────────────────────────

fn experiment() -> Workflow {
    Workflow {
        main_branch: "main".to_string(),
        develop_branch: None,
        supported_versions: None,
        ticket_pattern: None,
        types: vec![
            BranchType {
                name: "feature".to_string(),
                prefix: "feature/".to_string(),
                source: "main".to_string(),
                target: Some(BranchTarget::Single("main".to_string())),
                merge_strategy: MergeStrategy::Squash,
                delete_after_merge: Some(true),
                require_pr: Some(true),
                ..Default::default()
            },
            BranchType {
                name: "experiment".to_string(),
                prefix: "exp/".to_string(),
                source: "HEAD".to_string(), // From current position
                target: None,               // No default merge target
                merge_strategy: MergeStrategy::Rebase,
                ephemeral: Some(true),
                auto_cleanup_days: Some(30),
                ..Default::default()
            },
            BranchType {
                name: "spike".to_string(),
                prefix: "spike/".to_string(),
                source: "HEAD".to_string(),
                target: None, // Throwaway
                merge_strategy: MergeStrategy::Rebase,
                ephemeral: Some(true),
                auto_cleanup_days: Some(14),
                ..Default::default()
            },
        ],
        hooks: None,
        rules: Some(WorkflowRules {
            require_clean_tree: Some(true),
            ..Default::default()
        }),
    }
}

// ─── Ticket-Linked ─────────────────────────────────────────────────────────────

fn ticket_linked() -> Workflow {
    Workflow {
        main_branch: "main".to_string(),
        develop_branch: None,
        supported_versions: None,
        ticket_pattern: Some(r"[A-Z]+-\d+".to_string()), // JIRA-style: ABC-123
        types: vec![
            BranchType {
                name: "feature".to_string(),
                prefix: "feature/".to_string(),
                source: "main".to_string(),
                target: Some(BranchTarget::Single("main".to_string())),
                merge_strategy: MergeStrategy::Squash,
                delete_after_merge: Some(true),
                require_pr: Some(true),
                require_ticket: Some(true),
                pr_labels: Some(vec!["enhancement".to_string()]),
                ..Default::default()
            },
            BranchType {
                name: "fix".to_string(),
                prefix: "fix/".to_string(),
                source: "main".to_string(),
                target: Some(BranchTarget::Single("main".to_string())),
                merge_strategy: MergeStrategy::Squash,
                delete_after_merge: Some(true),
                require_pr: Some(true),
                require_ticket: Some(true),
                pr_labels: Some(vec!["bug".to_string()]),
                ..Default::default()
            },
            BranchType {
                name: "hotfix".to_string(),
                prefix: "hotfix/".to_string(),
                source: "main".to_string(),
                target: Some(BranchTarget::Single("main".to_string())),
                merge_strategy: MergeStrategy::Merge,
                delete_after_merge: Some(true),
                require_pr: Some(true),
                require_ticket: Some(true),
                pr_labels: Some(vec!["critical".to_string(), "bug".to_string()]),
                ..Default::default()
            },
        ],
        hooks: None,
        rules: Some(WorkflowRules {
            require_clean_tree: Some(true),
            require_up_to_date: Some(true),
            ..Default::default()
        }),
    }
}

// ─── Multi-Version ─────────────────────────────────────────────────────────────

fn multi_version() -> Workflow {
    Workflow {
        main_branch: "main".to_string(),
        develop_branch: None,
        supported_versions: Some(vec!["v2.x".to_string(), "v1.x".to_string()]),
        ticket_pattern: None,
        types: vec![
            BranchType {
                name: "feature".to_string(),
                prefix: "feature/".to_string(),
                source: "main".to_string(),
                target: Some(BranchTarget::Single("main".to_string())),
                merge_strategy: MergeStrategy::Squash,
                delete_after_merge: Some(true),
                require_pr: Some(true),
                ..Default::default()
            },
            BranchType {
                name: "hotfix".to_string(),
                prefix: "hotfix/".to_string(),
                source: "main".to_string(),
                target: Some(BranchTarget::Multiple(vec![
                    "main".to_string(),
                    "v2.x".to_string(),
                    "v1.x".to_string(),
                ])),
                merge_strategy: MergeStrategy::CherryPick,
                delete_after_merge: Some(true),
                tag_on_finish: Some(true),
                ..Default::default()
            },
            BranchType {
                name: "backport".to_string(),
                prefix: "backport/".to_string(),
                source: "main".to_string(),
                target: None, // User picks version branch at runtime
                merge_strategy: MergeStrategy::CherryPick,
                delete_after_merge: Some(true),
                ..Default::default()
            },
        ],
        hooks: None,
        rules: Some(WorkflowRules {
            require_clean_tree: Some(true),
            require_up_to_date: Some(true),
            ..Default::default()
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_presets_have_docs() {
        for name in preset_names() {
            assert!(
                get_preset(name).is_some(),
                "Preset '{}' has docs but no config",
                name
            );
            assert!(
                get_docs(name).is_some(),
                "Preset '{}' has config but no docs",
                name
            );
        }
    }

    #[test]
    fn test_gitflow_structure() {
        let workflow = gitflow();
        assert_eq!(workflow.main_branch, "main");
        assert_eq!(workflow.develop_branch, Some("develop".to_string()));
        assert_eq!(workflow.types.len(), 3);

        let feature = workflow.get_type("feature").unwrap();
        assert_eq!(feature.source, "develop");
        assert_eq!(feature.merge_strategy, MergeStrategy::Squash);
    }

    #[test]
    fn test_github_flow_simplicity() {
        let workflow = github_flow();
        assert!(workflow.develop_branch.is_none());
        assert_eq!(workflow.types.len(), 1);

        let feature = workflow.get_type("feature").unwrap();
        assert_eq!(feature.prefix, ""); // No prefix
        assert_eq!(feature.source, "main");
    }

    #[test]
    fn test_ticket_linked_validation() {
        let workflow = ticket_linked();
        assert!(workflow.ticket_pattern.is_some());

        let feature = workflow.get_type("feature").unwrap();
        assert_eq!(feature.require_ticket, Some(true));
    }

    #[test]
    fn test_multi_version_targets() {
        let workflow = multi_version();
        assert_eq!(
            workflow.supported_versions,
            Some(vec!["v2.x".to_string(), "v1.x".to_string()])
        );

        let hotfix = workflow.get_type("hotfix").unwrap();
        match &hotfix.target {
            Some(BranchTarget::Multiple(targets)) => {
                assert_eq!(targets.len(), 3);
                assert!(targets.contains(&"main".to_string()));
                assert!(targets.contains(&"v2.x".to_string()));
                assert!(targets.contains(&"v1.x".to_string()));
            }
            _ => panic!("Expected multiple targets for hotfix"),
        }
    }
}
