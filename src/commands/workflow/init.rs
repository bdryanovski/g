//! `g workflow init` — initialize workflow configuration.

use anyhow::Result;
use std::fs;
use std::io::{self, Write};

use crate::cli::workflow::InitArgs;
use crate::commands::Ctx;
use crate::config::workflow_presets;
use crate::config::{self, workflow::WorkflowsConfig};
use crate::ui::{self, print_section};

pub fn run(_ctx: &Ctx, args: InitArgs) -> Result<()> {
    if args.local {
        init_local(args)?;
    } else {
        init_global(args)?;
    }
    Ok(())
}

fn init_local(args: InitArgs) -> Result<()> {
    // Get repo root
    let repo_root = crate::commands::git::git_output(&["rev-parse", "--show-toplevel"])?;
    let g_dir = std::path::PathBuf::from(&repo_root).join(".g");
    let workflow_path = g_dir.join("workflow.toml");
    let gitignore_path = g_dir.join(".gitignore");

    // Check if already exists
    if workflow_path.exists() {
        ui::print_info(".g/workflow.toml already exists.");
        if !prompt_confirm("Overwrite existing configuration?", false)? {
            ui::print_info("Cancelled.");
            return Ok(());
        }
    }

    // Create .g directory
    if !g_dir.exists() {
        fs::create_dir_all(&g_dir)?;
        ui::print_info(&format!("Created {}", g_dir.display()));
    }

    // Ask about git tracking
    let track_in_git = if args.no_interactive {
        true // Default to tracking in non-interactive mode
    } else {
        prompt_confirm("Track .g/workflow.toml in git (share with team)?", true)?
    };

    // Handle .gitignore based on tracking preference
    if track_in_git {
        // Remove .gitignore if it exists and ignores everything
        if gitignore_path.exists() {
            let content = fs::read_to_string(&gitignore_path).unwrap_or_default();
            if content.trim() == "/*" || content.trim() == "*" {
                fs::remove_file(&gitignore_path)?;
                ui::print_info("Removed .g/.gitignore to enable tracking");
            }
        }
    } else {
        // Create .gitignore to exclude everything
        fs::write(
            &gitignore_path,
            "# Ignore all files in .g/ - local workflow config only\n/*\n",
        )?;
        ui::print_info("Created .g/.gitignore to prevent tracking");
    }

    // Determine workflow
    let workflow_name = if let Some(ref preset) = args.preset {
        preset.clone()
    } else if args.no_interactive {
        "github-flow".to_string()
    } else {
        select_preset()?
    };

    // Create config
    let mut workflows = WorkflowsConfig {
        default: Some(workflow_name.clone()),
        ..Default::default()
    };

    // Optionally include the preset inline
    let include_inline = if args.no_interactive {
        false
    } else {
        prompt_confirm(
            "Include workflow definition inline (allows customization)?",
            false,
        )?
    };

    if include_inline {
        if let Some(workflow) = workflow_presets::get_preset(&workflow_name) {
            workflows.workflows.insert(workflow_name.clone(), workflow);
        }
    }

    // Write config
    let content = generate_workflow_toml(&workflows, &workflow_name);
    fs::write(&workflow_path, content)?;

    ui::print_success(&format!("Created {}", workflow_path.display()));

    // Create templates directory
    let templates_dir = g_dir.join("templates");
    if !templates_dir.exists() {
        fs::create_dir_all(&templates_dir)?;

        // Create example PR template
        let feature_template = templates_dir.join("feature.md");
        if !feature_template.exists() {
            fs::write(&feature_template, PR_TEMPLATE_FEATURE)?;
            ui::print_info(&format!("Created {}", feature_template.display()));
        }
    }

    // Print next steps based on tracking choice
    println!();
    if track_in_git {
        ui::print_info("To share workflows with your team:");
        println!("  git add .g/");
        println!("  git commit -m \"chore: add workflow configuration\"");
    } else {
        ui::print_info("Workflow configuration is local-only (not tracked in git).");
        println!("To start tracking later, remove .g/.gitignore");
    }
    println!();

    Ok(())
}

fn init_global(args: InitArgs) -> Result<()> {
    let cfg = config::load()?;

    if !cfg.workflows.is_empty() {
        ui::print_info("Global workflows already configured.");

        // List existing
        println!();
        println!("Configured workflows:");
        for name in cfg.workflows.workflows.keys() {
            println!("  - {}", name);
        }
        if let Some(ref default) = cfg.workflows.default {
            println!();
            println!("Default: {}", default);
        }
        println!();

        if !prompt_confirm("Add another workflow?", false)? {
            return Ok(());
        }
    }

    // Select preset
    let workflow_name = if let Some(ref preset) = args.preset {
        preset.clone()
    } else if args.no_interactive {
        "github-flow".to_string()
    } else {
        select_preset()?
    };

    // Update config
    let mut cfg = cfg;
    if cfg.workflows.default.is_none() {
        cfg.workflows.default = Some(workflow_name.clone());
    }

    config::save(&cfg)?;

    ui::print_success(&format!("Workflow '{}' is now available", workflow_name));
    println!();
    println!("Usage:");
    println!(
        "  g workflow use {}      # Activate this workflow",
        workflow_name
    );
    println!("  g workflow start feature my-feature");
    println!();

    Ok(())
}

fn select_preset() -> Result<String> {
    println!();
    print_section("Select a Workflow", None);
    println!();

    let presets = workflow_presets::preset_names();

    for (i, name) in presets.iter().enumerate() {
        let docs = workflow_presets::get_docs(name);
        let desc = docs.map(|d| d.description).unwrap_or("");
        println!("  {}. {} - {}", i + 1, name, desc);
    }

    println!();
    print!("Choice [1-{}]: ", presets.len());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let idx: usize = input.trim().parse().unwrap_or(2); // Default to github-flow

    let selected = presets.get(idx.saturating_sub(1)).unwrap_or(&"github-flow");

    // Show diagram
    if let Some(docs) = workflow_presets::get_docs(selected) {
        println!();
        println!("{}", docs.diagram);
    }

    Ok(selected.to_string())
}

fn prompt_confirm(prompt: &str, default: bool) -> Result<bool> {
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    print!("{} {}: ", prompt, hint);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input.is_empty() {
        Ok(default)
    } else {
        Ok(input.starts_with('y'))
    }
}

fn generate_workflow_toml(workflows: &WorkflowsConfig, name: &str) -> String {
    let mut content = String::new();

    content.push_str("# g Workflow Configuration\n");
    content.push_str("# Documentation: https://g.dev/docs/workflows\n");
    content.push_str("#\n");
    content.push_str("# This file defines git workflows for this repository.\n");
    content.push_str("# Commit this file to share workflows with your team.\n");
    content.push_str("#\n");
    content.push_str("# Quick start:\n");
    content.push_str("#   g workflow start feature my-feature\n");
    content.push_str("#   g workflow sync\n");
    content.push_str("#   g workflow publish\n");
    content.push_str("#   g workflow finish\n");
    content.push_str("#\n");
    content.push('\n');

    // Add the actual config
    if let Ok(toml) = toml::to_string_pretty(workflows) {
        content.push_str(&toml);
    } else {
        // Minimal config
        content.push_str(&format!("default = \"{}\"\n", name));
    }

    content
}

const PR_TEMPLATE_FEATURE: &str = r#"## Description

Brief description of changes.

## Type of Change

- [ ] New feature
- [ ] Bug fix
- [ ] Refactoring
- [ ] Documentation

## Testing

- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual testing completed

## Checklist

- [ ] Code follows project style guidelines
- [ ] Self-review completed
- [ ] Documentation updated (if needed)
"#;
