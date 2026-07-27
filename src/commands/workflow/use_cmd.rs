//! `g workflow use <name>` — switch to a different workflow.

use anyhow::Result;

use crate::cli::workflow::UseArgs;
use crate::commands::workflow::shared::get_workflow;
use crate::commands::Ctx;
use crate::config::workflow_presets;
use crate::config::{self, workflow::WorkflowsConfig};
use crate::ui;

pub fn run(_ctx: &Ctx, args: UseArgs) -> Result<()> {
    // Verify the workflow exists
    let _ = get_workflow(&args.name)?;

    if args.local {
        // Save to repo-local config
        set_local_workflow(&args.name)?;
    } else {
        // Save to global config
        set_global_workflow(&args.name)?;
    }

    ui::print_success(&format!("Switched to workflow '{}'", args.name));

    // Show brief info about the workflow
    if let Some(docs) = workflow_presets::get_docs(&args.name) {
        println!();
        println!("{}", docs.description);
        println!();
        println!("Branch types: {}", docs.branch_types.join(", "));
    }

    Ok(())
}

fn set_local_workflow(name: &str) -> Result<()> {
    // Load existing local config or create new
    let mut workflows = match config::repo_workflow_path() {
        Ok(path) if path.exists() => {
            let raw = std::fs::read_to_string(&path)?;
            toml::from_str(&raw)?
        }
        _ => WorkflowsConfig::default(),
    };

    // Set default
    workflows.default = Some(name.to_string());

    // Save
    config::save_repo_workflows(&workflows)?;

    ui::print_info("Saved to .g/workflow.toml");
    Ok(())
}

fn set_global_workflow(name: &str) -> Result<()> {
    // Load global config
    let mut cfg = config::load()?;

    // Set default workflow
    cfg.workflows.default = Some(name.to_string());

    // Save
    config::save(&cfg)?;

    ui::print_info("Saved to global config");
    Ok(())
}
