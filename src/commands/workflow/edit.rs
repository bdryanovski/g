//! `g workflow edit` — edit an existing workflow configuration.

use anyhow::{bail, Result};
use std::fs;

use crate::cli::workflow::EditArgs;
use crate::commands::workflow::shared::{get_active_workflow, get_workflow};
use crate::commands::Ctx;
use crate::config::{self, workflow::WorkflowsConfig};
use crate::ui;

pub fn run(_ctx: &Ctx, args: EditArgs) -> Result<()> {
    // Get workflow name
    let name = if let Some(n) = args.name {
        n
    } else {
        // Use active workflow
        let (name, _) = get_active_workflow()?;
        name
    };

    // Get the workflow
    let workflow = get_workflow(&name)?;

    // Serialize to TOML
    let _toml_content = toml::to_string_pretty(&workflow)?;

    // Create a wrapper for the single workflow
    let mut workflows = WorkflowsConfig::default();
    workflows.workflows.insert(name.clone(), workflow);
    let full_content = toml::to_string_pretty(&workflows)?;

    // Get editor
    let editor = std::env::var("EDITOR")
        .unwrap_or_else(|_| std::env::var("VISUAL").unwrap_or_else(|_| "vim".to_string()));

    // Create temp file
    let temp_file = std::env::temp_dir().join(format!("g-workflow-{}.toml", name));
    fs::write(&temp_file, &full_content)?;

    ui::print_info(&format!("Opening '{}' in {}...", name, editor));

    // Open editor
    let status = std::process::Command::new(&editor)
        .arg(&temp_file)
        .status()?;

    if !status.success() {
        fs::remove_file(&temp_file)?;
        bail!("Editor exited with non-zero status");
    }

    // Read back and validate
    let edited = fs::read_to_string(&temp_file)?;
    fs::remove_file(&temp_file)?;

    // Parse and validate
    let parsed: WorkflowsConfig = match toml::from_str(&edited) {
        Ok(w) => w,
        Err(e) => {
            ui::print_error(&format!("Invalid TOML: {}", e));
            bail!("Configuration is invalid. Changes not saved.");
        }
    };

    // Get the edited workflow
    let edited_workflow = parsed
        .workflows
        .get(&name)
        .ok_or_else(|| anyhow::anyhow!("Workflow '{}' not found in edited content", name))?;

    // Determine where to save (check if it's in local config)
    let repo_path = config::repo_workflow_path().ok();
    let is_local = repo_path.as_ref().map(|p| p.exists()).unwrap_or(false);

    if is_local {
        // Check if this workflow is in local config
        let local_workflows: WorkflowsConfig = if let Some(ref path) = repo_path {
            if path.exists() {
                let raw = fs::read_to_string(path)?;
                toml::from_str(&raw)?
            } else {
                WorkflowsConfig::default()
            }
        } else {
            WorkflowsConfig::default()
        };

        if local_workflows.workflows.contains_key(&name) {
            // Save to local config
            let mut updated = local_workflows;
            updated
                .workflows
                .insert(name.clone(), edited_workflow.clone());
            config::save_repo_workflows(&updated)?;
            ui::print_success(&format!("Saved '{}' to .g/workflow.toml", name));
            return Ok(());
        }
    }

    // Save to global config
    let mut cfg = config::load()?;
    cfg.workflows
        .workflows
        .insert(name.clone(), edited_workflow.clone());
    config::save(&cfg)?;

    ui::print_success(&format!("Saved '{}' to global config", name));

    Ok(())
}
