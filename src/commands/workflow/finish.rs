//! `g workflow finish` — merge the current branch to its target(s).

use anyhow::{bail, Result};

use crate::cli::workflow::FinishArgs;
use crate::commands::git::{git_output, is_dry_run, require_clean_tree};
use crate::commands::workflow::hooks::{
    extract_ticket, run_post_finish, run_pre_finish, HookEnv,
};
use crate::commands::workflow::shared::{
    current_branch, detect_branch_type, get_active_workflow,
};
use crate::config::workflow::MergeStrategy;
use crate::commands::Ctx;
use crate::ui;

pub fn run(ctx: &Ctx, args: FinishArgs) -> Result<()> {
    let (workflow_name, workflow) = get_active_workflow()?;

    // Get the branch to finish
    let branch = args.branch.unwrap_or_else(|| current_branch().unwrap_or_default());
    if branch.is_empty() {
        bail!("Could not determine current branch. Are you in a git repository?");
    }

    // Detect branch type
    let branch_type = detect_branch_type(&workflow, &branch).ok_or_else(|| {
        anyhow::anyhow!(
            "Branch '{}' doesn't match any workflow type. \
             Use `g workflow status` to check the current workflow.",
            branch
        )
    })?;

    // Check for clean tree
    if !args.no_verify {
        require_clean_tree("finishing branch")?;
    }

    // Get target branch(es)
    let target = workflow.effective_target(branch_type).ok_or_else(|| {
        anyhow::anyhow!(
            "Branch type '{}' has no merge target configured. \
             This may be an experimental branch type.",
            branch_type.name
        )
    })?;

    // Determine merge strategy
    let strategy = if let Some(ref s) = args.strategy {
        match s.to_lowercase().as_str() {
            "merge" => MergeStrategy::Merge,
            "squash" => MergeStrategy::Squash,
            "rebase" => MergeStrategy::Rebase,
            "ff-only" | "ff" => MergeStrategy::FfOnly,
            "cherry-pick" | "cherry" => MergeStrategy::CherryPick,
            _ => bail!("Unknown merge strategy: {}", s),
        }
    } else {
        branch_type.merge_strategy.clone()
    };

    // Get source for hooks
    let source = workflow.effective_source(branch_type);

    // Create hook environment
    let ticket = extract_ticket(&workflow, &branch);
    let hook_env = HookEnv::new(
        &workflow_name,
        branch_type,
        &branch,
        source,
        &target.to_string(),
    )
    .with_ticket(ticket);

    // Run pre-finish hooks
    if !args.no_verify {
        run_pre_finish(&workflow.hooks, &hook_env, is_dry_run())?;
    }

    // Perform merge(s)
    let targets = target.as_slice();
    for target_branch in &targets {
        merge_to_target(ctx, &branch, target_branch, &strategy)?;
    }

    // Create tag if configured
    if branch_type.tag_on_finish == Some(true) && !args.no_tag {
        create_tag(ctx, branch_type, &branch)?;
    }

    // Delete branch if configured
    let should_delete = branch_type.delete_after_merge.unwrap_or(false) && !args.no_delete;
    if should_delete {
        delete_branch(ctx, &branch)?;
    }

    // Run post-finish hooks
    run_post_finish(&workflow.hooks, &hook_env, is_dry_run())?;

    // Print summary
    println!();
    ui::print_success(&format!(
        "Finished '{}' -> {}",
        branch,
        targets.join(", ")
    ));

    if should_delete {
        ui::print_info(&format!("Branch '{}' has been deleted.", branch));
    }

    Ok(())
}

fn merge_to_target(
    _ctx: &Ctx,
    branch: &str,
    target: &str,
    strategy: &MergeStrategy,
) -> Result<()> {
    ui::print_info(&format!(
        "Merging '{}' into '{}' using {} strategy...",
        branch,
        target,
        strategy
    ));

    if is_dry_run() {
        ui::print_info(&format!(
            "Would merge '{}' into '{}' using {:?}",
            branch, target, strategy
        ));
        return Ok(());
    }

    // Checkout target branch
    git_output(&["checkout", target])?;

    // Pull latest
    let _ = git_output(&["pull", "--ff-only", "origin", target]);

    // Perform merge based on strategy
    match strategy {
        MergeStrategy::Merge => {
            git_output(&["merge", "--no-ff", branch, "-m", &format!("Merge branch '{}'", branch)])?;
        }
        MergeStrategy::Squash => {
            git_output(&["merge", "--squash", branch])?;
            git_output(&["commit", "-m", &format!("Squash merge branch '{}'", branch)])?;
        }
        MergeStrategy::Rebase => {
            // For rebase, we rebase the branch onto target, then fast-forward target
            git_output(&["checkout", branch])?;
            git_output(&["rebase", target])?;
            git_output(&["checkout", target])?;
            git_output(&["merge", "--ff-only", branch])?;
        }
        MergeStrategy::FfOnly => {
            git_output(&["merge", "--ff-only", branch])?;
        }
        MergeStrategy::CherryPick => {
            // Get commits to cherry-pick
            let merge_base = git_output(&["merge-base", target, branch])?;
            let commits = git_output(&["rev-list", "--reverse", &format!("{}..{}", merge_base, branch)])?;

            for commit in commits.lines() {
                if !commit.is_empty() {
                    git_output(&["cherry-pick", commit])?;
                }
            }
        }
    }

    // Push to remote
    ui::print_info(&format!("Pushing '{}'...", target));
    git_output(&["push", "origin", target])?;

    Ok(())
}

fn create_tag(_ctx: &Ctx, branch_type: &crate::config::workflow::BranchType, branch: &str) -> Result<()> {
    let pattern = branch_type
        .tag_pattern
        .as_ref()
        .map(|p| p.as_str())
        .unwrap_or("{name}");

    // Extract name from branch (without prefix)
    let name = if branch_type.prefix.is_empty() {
        branch.to_string()
    } else {
        branch
            .strip_prefix(&branch_type.prefix)
            .unwrap_or(branch)
            .to_string()
    };

    // Replace variables in pattern
    let tag = pattern
        .replace("{name}", &name)
        .replace("{type}", &branch_type.name)
        .replace("{date}", &chrono::Local::now().format("%Y-%m-%d").to_string());

    // Handle {version} - try to extract from name
    let tag = if tag.contains("{version}") {
        // Try to find version-like pattern in name (e.g., "1.2.3", "v1.0")
        let version = extract_version(&name).unwrap_or_else(|| name.clone());
        tag.replace("{version}", &version)
    } else {
        tag
    };

    if is_dry_run() {
        ui::print_info(&format!("Would create tag: {}", tag));
        return Ok(());
    }

    ui::print_info(&format!("Creating tag: {}", tag));
    git_output(&["tag", "-a", &tag, "-m", &format!("Release {}", tag)])?;
    git_output(&["push", "origin", &tag])?;

    Ok(())
}

fn delete_branch(_ctx: &Ctx, branch: &str) -> Result<()> {
    if is_dry_run() {
        ui::print_info(&format!("Would delete branch: {}", branch));
        return Ok(());
    }

    // Delete local branch
    git_output(&["branch", "-d", branch])?;

    // Delete remote branch (if exists)
    let _ = git_output(&["push", "origin", "--delete", branch]);

    Ok(())
}

/// Try to extract a version number from a string.
fn extract_version(s: &str) -> Option<String> {
    let re = regex::Regex::new(r"v?(\d+\.\d+(?:\.\d+)?)").ok()?;
    re.captures(s)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}
