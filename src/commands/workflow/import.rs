//! `g workflow import` — import workflow from a TOML file.

use anyhow::{bail, Result};
use std::fs;

use crate::cli::workflow::ImportArgs;
use crate::commands::workflow::shared::load_workflows;
use crate::config::{self, workflow::WorkflowsConfig};
use crate::config::workflow_presets;
use crate::commands::Ctx;
use crate::ui;

pub fn run(_ctx: &Ctx, args: ImportArgs) -> Result<()> {
    // Read the file
    let content = fs::read_to_string(&args.file)?;

    // Parse TOML
    let imported: WorkflowsConfig = toml::from_str(&content)?;

    if imported.workflows.is_empty() {
        bail!("No workflows found in '{}'", args.file);
    }

    // Determine which workflows to import
    let workflows_to_import: Vec<(String, _)> = if let Some(ref name) = args.name {
        // Import with a specific name (use first workflow from file)
        let (_, workflow) = imported.workflows.into_iter().next().unwrap();
        vec![(name.clone(), workflow)]
    } else {
        // Import all workflows from file
        imported.workflows.into_iter().collect()
    };

    // Check for conflicts
    let existing = load_workflows()?;
    for (name, _) in &workflows_to_import {
        if existing.get(name).is_some() || workflow_presets::get_preset(name).is_some() {
            bail!(
                "Workflow '{}' already exists. Use --name to import with a different name.",
                name
            );
        }
    }

    // Import workflows
    if args.local {
        // Save to repo-local config
        let mut local_config = match config::repo_workflow_path() {
            Ok(path) if path.exists() => {
                let raw = fs::read_to_string(&path)?;
                toml::from_str(&raw)?
            }
            _ => WorkflowsConfig::default(),
        };

        for (name, workflow) in workflows_to_import {
            ui::print_info(&format!("Importing '{}'...", name));
            local_config.workflows.insert(name, workflow);
        }

        config::save_repo_workflows(&local_config)?;
        ui::print_success("Imported to .g/workflow.toml");
    } else {
        // Save to global config
        let mut cfg = config::load()?;

        for (name, workflow) in workflows_to_import {
            ui::print_info(&format!("Importing '{}'...", name));
            cfg.workflows.workflows.insert(name, workflow);
        }

        config::save(&cfg)?;
        ui::print_success("Imported to global config");
    }

    println!();
    println!("Use `g workflow list` to see available workflows.");
    println!();

    Ok(())
}
