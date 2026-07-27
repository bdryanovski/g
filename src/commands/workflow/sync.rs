//! `g workflow sync` — update branch from its source.

use anyhow::{bail, Result};

use crate::cli::workflow::SyncArgs;
use crate::commands::git::{git_output, is_dry_run, require_clean_tree};
use crate::commands::workflow::shared::{
    commits_ahead_behind, current_branch, detect_branch_type, get_active_workflow,
};
use crate::commands::Ctx;
use crate::config::workflow::MergeStrategy;
use crate::ui;

pub fn run(ctx: &Ctx, args: SyncArgs) -> Result<()> {
    let (_, workflow) = get_active_workflow()?;

    // Get the branch to sync
    let branch = args
        .branch
        .unwrap_or_else(|| current_branch().unwrap_or_default());
    if branch.is_empty() {
        bail!("Could not determine current branch. Are you in a git repository?");
    }

    // Detect branch type
    let branch_type = detect_branch_type(&workflow, &branch);

    // Determine source branch
    let source = if let Some(bt) = branch_type {
        workflow.effective_source(bt).to_string()
    } else {
        // Fallback to main branch if type not detected
        workflow.main_branch.clone()
    };

    // Check for clean tree
    require_clean_tree("syncing branch")?;

    // Determine update strategy
    let use_rebase = if args.rebase {
        true
    } else if args.merge {
        false
    } else if let Some(bt) = branch_type {
        // Use branch type's merge strategy
        matches!(
            bt.merge_strategy,
            MergeStrategy::Rebase | MergeStrategy::FfOnly
        )
    } else {
        // Default to rebase for clean history
        true
    };

    // Make sure we're on the right branch
    let current = current_branch()?;
    if current != branch {
        ui::print_info(&format!("Checking out '{}'...", branch));
        if !is_dry_run() {
            git_output(&["checkout", &branch])?;
        }
    }

    // Fetch latest (ignore errors for local-only repos)
    ui::print_info("Fetching latest from origin...");
    if !is_dry_run() && git_output(&["fetch", "origin"]).is_err() {
        ui::print_warning("Could not fetch from origin (no remote configured?)");
    }

    // Check how far behind we are
    let remote_source = format!("origin/{}", source);
    let (ahead, behind) = commits_ahead_behind(&branch, &remote_source).unwrap_or((0, 0));

    if behind == 0 {
        ui::print_success(&format!(
            "Branch '{}' is already up to date with '{}'.",
            branch, source
        ));
        if ahead > 0 {
            ui::print_info(&format!("You are {} commit(s) ahead.", ahead));
        }
        return Ok(());
    }

    ui::print_info(&format!(
        "Branch '{}' is {} commit(s) behind '{}'.",
        branch, behind, source
    ));

    // Perform sync
    if use_rebase {
        rebase_sync(ctx, &branch, &source)?;
    } else {
        merge_sync(ctx, &branch, &source)?;
    }

    Ok(())
}

fn rebase_sync(_ctx: &Ctx, branch: &str, source: &str) -> Result<()> {
    ui::print_info(&format!("Rebasing '{}' onto '{}'...", branch, source));

    if is_dry_run() {
        ui::print_info(&format!(
            "Would rebase '{}' onto 'origin/{}'",
            branch, source
        ));
        return Ok(());
    }

    let result = git_output(&["rebase", &format!("origin/{}", source)]);

    match result {
        Ok(_) => {
            ui::print_success(&format!(
                "Successfully rebased '{}' onto '{}'.",
                branch, source
            ));
            Ok(())
        }
        Err(e) => {
            // Check if there's a rebase in progress
            if git_output(&["rebase", "--show-current-patch"]).is_ok() {
                ui::print_warning("Rebase conflict detected!");
                println!();
                println!("Resolve conflicts, then:");
                println!("  git add <files>");
                println!("  git rebase --continue");
                println!();
                println!("Or abort with:");
                println!("  git rebase --abort");
                bail!("Rebase paused due to conflicts.");
            }
            Err(e)
        }
    }
}

fn merge_sync(_ctx: &Ctx, branch: &str, source: &str) -> Result<()> {
    ui::print_info(&format!("Merging '{}' into '{}'...", source, branch));

    if is_dry_run() {
        ui::print_info(&format!(
            "Would merge 'origin/{}' into '{}'",
            source, branch
        ));
        return Ok(());
    }

    let result = git_output(&[
        "merge",
        &format!("origin/{}", source),
        "-m",
        &format!("Merge '{}' into '{}'", source, branch),
    ]);

    match result {
        Ok(_) => {
            ui::print_success(&format!(
                "Successfully merged '{}' into '{}'.",
                source, branch
            ));
            Ok(())
        }
        Err(_e) => {
            ui::print_warning("Merge conflict detected!");
            println!();
            println!("Resolve conflicts, then:");
            println!("  git add <files>");
            println!("  git commit");
            println!();
            println!("Or abort with:");
            println!("  git merge --abort");
            bail!("Merge paused due to conflicts.");
        }
    }
}
