//! `g workflow start <type> <name>` — create a new branch using workflow rules.

use anyhow::{bail, Result};

use crate::cli::workflow::StartArgs;
use crate::commands::git::{git_output, is_dry_run};
use crate::commands::workflow::hooks::{extract_ticket, run_post_start, run_pre_start, HookEnv};
use crate::commands::workflow::shared::{
    branch_exists, get_active_workflow, make_branch_name, resolve_source_branch, verify_rules,
};
use crate::commands::Ctx;
use crate::ui;

pub fn run(_ctx: &Ctx, args: StartArgs) -> Result<()> {
    let (workflow_name, workflow) = get_active_workflow()?;

    // Find the branch type
    let branch_type = workflow.get_type(&args.branch_type).ok_or_else(|| {
        let available: Vec<_> = workflow.types.iter().map(|t| t.name.as_str()).collect();
        anyhow::anyhow!(
            "Unknown branch type '{}'. Available types: {}",
            args.branch_type,
            available.join(", ")
        )
    })?;

    // Validate branch name
    if !args.no_verify {
        workflow
            .validate_branch_name(branch_type, &args.name)
            .map_err(|e| anyhow::anyhow!(e))?;
    }

    // Verify workflow rules (skip in dry run)
    if !is_dry_run() {
        verify_rules(&workflow, "starting a new branch")?;
    }

    // Resolve source branch
    let source = if let Some(ref from) = args.from {
        // User override
        if !branch_exists(from)? {
            bail!("Source branch '{}' does not exist.", from);
        }
        from.clone()
    } else {
        resolve_source_branch(&workflow, branch_type)?
    };

    // Construct full branch name
    let branch_name = make_branch_name(branch_type, &args.name);

    // Check if branch already exists
    if branch_exists(&branch_name)? {
        bail!(
            "Branch '{}' already exists. Choose a different name or delete the existing branch.",
            branch_name
        );
    }

    // Get target for hooks
    let target = workflow
        .effective_target(branch_type)
        .map(|t| t.to_string())
        .unwrap_or_default();

    // Create hook environment
    let ticket = extract_ticket(&workflow, &branch_name);
    let hook_env = HookEnv::new(&workflow_name, branch_type, &branch_name, &source, &target)
        .with_ticket(ticket);

    // Run pre-start hooks
    run_pre_start(&workflow.hooks, &hook_env, is_dry_run())?;

    // Fetch latest from remote
    ui::print_info("Fetching latest from origin...");
    let _ = git_output(&["fetch", "origin", &source]);

    // Create and checkout the branch
    if is_dry_run() {
        ui::print_info(&format!(
            "Would create branch '{}' from '{}'",
            branch_name, source
        ));
    } else {
        ui::print_info(&format!(
            "Creating branch '{}' from '{}'...",
            branch_name, source
        ));

        // Update source branch if it exists on remote
        let remote_ref = format!("origin/{}", source);
        if git_output(&["rev-parse", "--verify", &remote_ref]).is_ok() {
            // Checkout source and update it
            git_output(&["checkout", &source])?;
            git_output(&["pull", "--ff-only", "origin", &source])?;
        }

        // Create and checkout new branch
        git_output(&["checkout", "-b", &branch_name])?;

        ui::print_success(&format!("Created and switched to '{}'", branch_name));
    }

    // Run post-start hooks
    run_post_start(&workflow.hooks, &hook_env, is_dry_run())?;

    // Print next steps
    println!();
    ui::print_info("Next steps:");
    println!("  1. Make your changes and commit them");
    println!("  2. Run `g workflow sync` to update from {}", source);
    println!("  3. Run `g workflow publish` to push and create a PR");
    println!("  4. Run `g workflow finish` to merge when ready");

    Ok(())
}
