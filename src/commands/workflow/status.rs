//! `g workflow status` — show workflow status of current branch.

use anyhow::{bail, Result};

use crate::commands::git::git_output;
use crate::commands::workflow::shared::{
    commits_ahead_behind, current_branch, detect_branch_type, get_active_workflow,
};
use crate::commands::Ctx;
use crate::config::workflow_presets;
use crate::ui::{self, print_section};

pub fn run(_ctx: &Ctx) -> Result<()> {
    let (workflow_name, workflow) = get_active_workflow()?;

    // Get current branch
    let branch = current_branch()?;
    if branch.is_empty() {
        bail!("Could not determine current branch. Are you in a git repository?");
    }

    // Detect branch type
    let branch_type = detect_branch_type(&workflow, &branch);

    // Print header
    println!();
    print_section("Current Branch", None);
    println!();

    // Branch info
    println!("  Branch:    {}", branch);
    println!("  Workflow:  {}", workflow_name);

    if let Some(bt) = branch_type {
        println!("  Type:      {}", bt.name);
        println!();

        // Source/target info
        let source = workflow.effective_source(bt);
        let target = workflow
            .effective_target(bt)
            .map(|t| t.to_string())
            .unwrap_or_else(|| "(none)".to_string());

        // Check ahead/behind
        let remote_source = format!("origin/{}", source);
        let (_ahead, behind) = commits_ahead_behind(&branch, &remote_source).unwrap_or((0, 0));

        let source_status = if behind > 0 {
            format!("{} ({} commits behind)", source, behind)
        } else {
            format!("{} (up to date)", source)
        };

        println!("  Source:    {}", source_status);
        println!("  Target:    {}", target);
        println!("  Strategy:  {}", bt.merge_strategy);
        println!();

        // Branch age
        if let Ok(created) = git_output(&["log", "--format=%ci", "--reverse", &branch, "-1"]) {
            if !created.is_empty() {
                println!(
                    "  Created:   {}",
                    created.split_whitespace().next().unwrap_or(&created)
                );
            }
        }

        // Commit count on this branch
        if let Ok(merge_base) = git_output(&["merge-base", source, &branch]) {
            if let Ok(count) = git_output(&[
                "rev-list",
                "--count",
                &format!("{}..{}", merge_base, branch),
            ]) {
                println!("  Commits:   {}", count);
            }
        }

        // PR status
        print_pr_status(&branch);

        // Warnings
        println!();
        if behind > 0 {
            ui::print_warning(&format!(
                "Branch is {} commits behind '{}'. Run `g workflow sync`.",
                behind, source
            ));
        }

        if let Some(max_hours) = bt.max_age_hours {
            // Check age warning for trunk-based
            if let Ok(age_days) = get_branch_age_days(&branch) {
                let max_days = max_hours / 24;
                if age_days > max_days as i64 {
                    ui::print_warning(&format!(
                        "Branch is {} days old. Consider finishing or rebasing.",
                        age_days
                    ));
                }
            }
        }
    } else {
        println!("  Type:      (not matched to any workflow type)");
        println!();
        ui::print_info(&format!(
            "This branch doesn't match any type in the '{}' workflow.",
            workflow_name
        ));
        println!();

        // Show available types
        println!("  Available branch types:");
        for bt in &workflow.types {
            let prefix = if bt.prefix.is_empty() {
                "(no prefix)".to_string()
            } else {
                bt.prefix.to_string()
            };
            println!("    {} -> {}", bt.name, prefix);
        }
    }

    // Show workflow diagram if available
    if let Some(docs) = workflow_presets::get_docs(&workflow_name) {
        println!();
        print_section(&format!("Workflow: {}", workflow_name), None);
        println!("{}", docs.diagram);
    }

    // Next actions
    println!();
    print_section("Actions", None);
    println!();
    println!("  g workflow sync      Update from source branch");
    println!("  g workflow publish   Push and create/update PR");
    println!("  g workflow finish    Merge to target branch(es)");
    println!();

    Ok(())
}

fn print_pr_status(branch: &str) {
    // Try to get PR info using gh CLI
    let gh_available = std::process::Command::new("gh")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !gh_available {
        return;
    }

    let output = std::process::Command::new("gh")
        .args([
            "pr",
            "view",
            branch,
            "--json",
            "number,state,url,reviewDecision",
        ])
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let json = String::from_utf8_lossy(&output.stdout);
            if let Ok(pr) = serde_json::from_str::<serde_json::Value>(&json) {
                let number = pr["number"].as_i64().unwrap_or(0);
                let state = pr["state"].as_str().unwrap_or("unknown");
                let url = pr["url"].as_str().unwrap_or("");
                let review = pr["reviewDecision"].as_str().unwrap_or("");

                let status = match state {
                    "OPEN" => {
                        let review_status = match review {
                            "APPROVED" => " (approved)",
                            "CHANGES_REQUESTED" => " (changes requested)",
                            "REVIEW_REQUIRED" => " (review required)",
                            _ => "",
                        };
                        format!("#{} open{}", number, review_status)
                    }
                    "MERGED" => format!("#{} merged", number),
                    "CLOSED" => format!("#{} closed", number),
                    _ => format!("#{}", number),
                };

                println!("  PR:        {}", status);
                if !url.is_empty() {
                    println!("  URL:       {}", url);
                }
            }
        }
    }
}

fn get_branch_age_days(branch: &str) -> Result<i64> {
    let output = git_output(&["log", "--format=%ct", "--reverse", branch, "-1"])?;
    let timestamp: i64 = output.trim().parse()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    Ok((now - timestamp) / 86400)
}
