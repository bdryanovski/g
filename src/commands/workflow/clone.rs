//! `g workflow clone` — clone a workflow with a new name.

use anyhow::{bail, Result};

use crate::cli::workflow::CloneArgs;
use crate::commands::workflow::shared::{get_workflow, load_workflows};
use crate::commands::Ctx;
use crate::config::workflow_presets;
use crate::config::{self};
use crate::ui;

pub fn run(_ctx: &Ctx, args: CloneArgs) -> Result<()> {
    // Check source exists
    let source_workflow = get_workflow(&args.source)?;

    // Check target doesn't exist
    let workflows = load_workflows()?;
    if workflows.get(&args.name).is_some() || workflow_presets::get_preset(&args.name).is_some() {
        bail!(
            "Workflow '{}' already exists. Choose a different name.",
            args.name
        );
    }

    // Clone the workflow
    let cloned = source_workflow.clone();

    // Save to global config
    let mut cfg = config::load()?;
    cfg.workflows.workflows.insert(args.name.clone(), cloned);
    config::save(&cfg)?;

    ui::print_success(&format!("Cloned '{}' as '{}'", args.source, args.name));

    println!();
    println!("Next steps:");
    println!(
        "  g workflow edit {}    # Customize the workflow",
        args.name
    );
    println!("  g workflow use {}     # Activate it", args.name);
    println!();

    Ok(())
}
