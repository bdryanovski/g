//! `g workflow publish` — push branch and create/update PR.

use anyhow::{bail, Result};

use crate::cli::workflow::PublishArgs;
use crate::commands::git::{git_output, is_dry_run};
use crate::commands::workflow::hooks::{extract_ticket, run_on_publish, HookEnv};
use crate::commands::workflow::shared::{
    current_branch, detect_branch_type, extract_branch_name, get_active_workflow,
    remote_branch_exists,
};
use crate::commands::Ctx;
use crate::ui;

pub fn run(_ctx: &Ctx, args: PublishArgs) -> Result<()> {
    let (workflow_name, workflow) = get_active_workflow()?;

    // Get the branch to publish
    let branch = args.branch.clone().unwrap_or_else(|| current_branch().unwrap_or_default());
    if branch.is_empty() {
        bail!("Could not determine current branch. Are you in a git repository?");
    }

    // Detect branch type
    let branch_type = detect_branch_type(&workflow, &branch);

    // Get target for PR base
    let target = if let Some(bt) = branch_type {
        workflow
            .effective_target(bt)
            .map(|t| t.primary().to_string())
            .unwrap_or_else(|| workflow.main_branch.clone())
    } else {
        workflow.main_branch.clone()
    };

    // Get source for hooks
    let source = if let Some(bt) = branch_type {
        workflow.effective_source(bt).to_string()
    } else {
        workflow.main_branch.clone()
    };

    // Prepare hook environment
    let ticket = extract_ticket(&workflow, &branch);
    let hook_env = if let Some(bt) = branch_type {
        HookEnv::new(&workflow_name, bt, &branch, &source, &target).with_ticket(ticket.clone())
    } else {
        // Create minimal env for non-typed branches
        HookEnv {
            workflow: workflow_name.clone(),
            branch_type: "unknown".to_string(),
            branch_name: branch.clone(),
            source,
            target: target.clone(),
            ticket: ticket.clone(),
        }
    };

    // Run on_publish hooks
    if !args.no_verify {
        run_on_publish(&workflow.hooks, &hook_env, is_dry_run())?;
    }

    // Push branch
    push_branch(&branch)?;

    // Create or update PR
    create_or_update_pr(&branch, &target, &args, branch_type, &ticket)?;

    Ok(())
}

fn push_branch(branch: &str) -> Result<()> {
    ui::print_info(&format!("Pushing '{}'...", branch));

    if is_dry_run() {
        ui::print_info(&format!("Would push '{}' to origin", branch));
        return Ok(());
    }

    // Check if remote branch exists
    let remote_exists = remote_branch_exists(branch)?;

    if remote_exists {
        // Force push with lease for safety
        git_output(&["push", "--force-with-lease", "origin", branch])?;
    } else {
        // First push, set upstream
        git_output(&["push", "-u", "origin", branch])?;
    }

    ui::print_success(&format!("Pushed '{}'", branch));
    Ok(())
}

fn create_or_update_pr(
    branch: &str,
    target: &str,
    args: &PublishArgs,
    branch_type: Option<&crate::config::workflow::BranchType>,
    ticket: &Option<String>,
) -> Result<()> {
    // Try to use gh CLI if available
    let gh_available = std::process::Command::new("gh")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !gh_available {
        ui::print_info("GitHub CLI (gh) not found. Skipping PR creation.");
        ui::print_info(&format!(
            "Create a PR manually: https://github.com/<owner>/<repo>/compare/{}...{}",
            target, branch
        ));
        return Ok(());
    }

    // Check if PR already exists
    let pr_exists = std::process::Command::new("gh")
        .args(["pr", "view", branch, "--json", "number"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if pr_exists {
        ui::print_info("PR already exists for this branch.");

        // Show PR URL
        if let Ok(output) = std::process::Command::new("gh")
            .args(["pr", "view", branch, "--json", "url", "-q", ".url"])
            .output()
        {
            if output.status.success() {
                let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !url.is_empty() {
                    ui::print_success(&format!("PR: {}", url));
                }
            }
        }
        return Ok(());
    }

    // Determine PR title
    let title = args.title.clone().unwrap_or_else(|| {
        // Generate title from branch name
        if let Some(bt) = branch_type {
            let name = extract_branch_name(bt, branch);
            format_pr_title(&bt.name, &name, ticket)
        } else {
            branch.replace(['/', '-', '_'], " ")
        }
    });

    // Determine PR body
    let body = args.body.clone().unwrap_or_else(|| {
        let mut body = String::new();

        if let Some(ref t) = ticket {
            body.push_str(&format!("Closes {}\n\n", t));
        }

        if let Some(bt) = branch_type {
            body.push_str(&format!("## Type\n{}\n\n", bt.name));
        }

        body.push_str("## Changes\n\n- \n\n");
        body.push_str("## Testing\n\n- [ ] Tests pass\n");

        body
    });

    if is_dry_run() {
        ui::print_info("Would create PR:");
        println!("  Title: {}", title);
        println!("  Base: {}", target);
        println!("  Draft: {}", args.draft);
        return Ok(());
    }

    ui::print_info("Creating PR...");

    // Build gh pr create command
    let mut gh_args = vec![
        "pr".to_string(),
        "create".to_string(),
        "--base".to_string(),
        target.to_string(),
        "--title".to_string(),
        title.clone(),
        "--body".to_string(),
        body,
    ];

    if args.draft {
        gh_args.push("--draft".to_string());
    }

    // Add reviewers
    if let Some(ref reviewers) = args.reviewers {
        for reviewer in reviewers.split(',') {
            gh_args.push("--reviewer".to_string());
            gh_args.push(reviewer.trim().to_string());
        }
    } else if let Some(bt) = branch_type {
        if let Some(ref reviewers) = bt.pr_reviewers {
            for reviewer in reviewers {
                gh_args.push("--reviewer".to_string());
                gh_args.push(reviewer.clone());
            }
        }
    }

    // Add labels
    if let Some(ref labels) = args.labels {
        for label in labels.split(',') {
            gh_args.push("--label".to_string());
            gh_args.push(label.trim().to_string());
        }
    } else if let Some(bt) = branch_type {
        if let Some(ref labels) = bt.pr_labels {
            for label in labels {
                gh_args.push("--label".to_string());
                gh_args.push(label.clone());
            }
        }
    }

    let output = std::process::Command::new("gh")
        .args(&gh_args)
        .output()?;

    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        ui::print_success(&format!("Created PR: {}", url));
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to create PR: {}", stderr);
    }

    Ok(())
}

/// Format a PR title from branch type and name.
fn format_pr_title(branch_type: &str, name: &str, ticket: &Option<String>) -> String {
    let formatted_name = name.replace(['-', '_'], " ");

    if let Some(ref t) = ticket {
        format!("{}: {} [{}]", branch_type, formatted_name, t)
    } else {
        format!("{}: {}", branch_type, formatted_name)
    }
}
