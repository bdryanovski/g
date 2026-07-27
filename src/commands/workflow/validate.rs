//! `g workflow validate` — validate workflow configuration.

use anyhow::Result;
use std::fs;

use crate::cli::workflow::ValidateArgs;
use crate::commands::workflow::shared::{branch_exists, load_workflows};
use crate::config::workflow::{Workflow, WorkflowsConfig};
use crate::config::workflow_presets;
use crate::commands::Ctx;
use crate::ui::{self, print_section};

pub fn run(_ctx: &Ctx, args: ValidateArgs) -> Result<()> {
    let workflows = if let Some(ref file) = args.file {
        // Load from file
        let content = fs::read_to_string(file)?;
        let config: WorkflowsConfig = toml::from_str(&content)?;
        config
    } else {
        load_workflows()?
    };

    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Check if any workflows are defined
    if workflows.is_empty() {
        warnings.push("No workflows defined. Using built-in presets.".to_string());
    }

    // Validate each workflow
    let workflow_names: Vec<_> = if let Some(ref name) = args.workflow {
        vec![name.clone()]
    } else {
        workflows.workflows.keys().cloned().collect()
    };

    for name in &workflow_names {
        let workflow = if let Some(w) = workflows.get(name) {
            w.clone()
        } else if let Some(w) = workflow_presets::get_preset(name) {
            w
        } else {
            errors.push(format!("Workflow '{}' not found", name));
            continue;
        };

        validate_workflow(name, &workflow, &mut errors, &mut warnings);
    }

    // Check default workflow exists
    if let Some(ref default) = workflows.default {
        if workflows.get(default).is_none() && workflow_presets::get_preset(default).is_none() {
            errors.push(format!(
                "Default workflow '{}' not found",
                default
            ));
        }
    }

    // Print results
    println!();
    print_section("Validation Results", None);
    println!();

    if errors.is_empty() && warnings.is_empty() {
        ui::print_success("Configuration is valid!");
        println!();

        // Print summary
        println!("Workflows validated:");
        for name in &workflow_names {
            let workflow = workflows.get(name).cloned().or_else(|| workflow_presets::get_preset(name));
            if let Some(w) = workflow {
                let types: Vec<_> = w.types.iter().map(|t| t.name.as_str()).collect();
                println!("  {} - {} type(s): {}", name, types.len(), types.join(", "));
            }
        }
        println!();
    } else {
        // Print errors
        if !errors.is_empty() {
            ui::print_error(&format!("{} error(s) found:", errors.len()));
            for err in &errors {
                println!("  - {}", err);
            }
            println!();
        }

        // Print warnings
        if !warnings.is_empty() {
            ui::print_warning(&format!("{} warning(s):", warnings.len()));
            for warn in &warnings {
                println!("  - {}", warn);
            }
            println!();
        }

        if !errors.is_empty() {
            std::process::exit(1);
        }
    }

    Ok(())
}

fn validate_workflow(
    name: &str,
    workflow: &Workflow,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    // Check main branch
    if workflow.main_branch.is_empty() {
        errors.push(format!("[{}] Main branch is empty", name));
    } else if !branch_exists(&workflow.main_branch).unwrap_or(false) {
        warnings.push(format!(
            "[{}] Main branch '{}' doesn't exist locally",
            name, workflow.main_branch
        ));
    }

    // Check develop branch
    if let Some(ref develop) = workflow.develop_branch {
        if !branch_exists(develop).unwrap_or(false) {
            warnings.push(format!(
                "[{}] Develop branch '{}' doesn't exist locally",
                name, develop
            ));
        }
    }

    // Check branch types
    if workflow.types.is_empty() {
        warnings.push(format!("[{}] No branch types defined", name));
    }

    let mut seen_names = std::collections::HashSet::new();
    let mut seen_prefixes = std::collections::HashSet::new();

    for bt in &workflow.types {
        // Check for duplicate names
        if !seen_names.insert(&bt.name) {
            errors.push(format!(
                "[{}] Duplicate branch type name: '{}'",
                name, bt.name
            ));
        }

        // Check for duplicate prefixes (if non-empty)
        if !bt.prefix.is_empty() && !seen_prefixes.insert(&bt.prefix) {
            errors.push(format!(
                "[{}] Duplicate branch prefix: '{}'",
                name, bt.prefix
            ));
        }

        // Validate naming pattern regex
        if let Some(ref pattern) = bt.naming_pattern {
            if regex::Regex::new(pattern).is_err() {
                errors.push(format!(
                    "[{}] Invalid regex in naming_pattern for '{}': {}",
                    name, bt.name, pattern
                ));
            }
        }

        // Check source branch reference
        let source = &bt.source;
        if source != "HEAD" && source != "main" && source != "develop" && !source.contains('*') {
            if !branch_exists(source).unwrap_or(false) {
                warnings.push(format!(
                    "[{}] Source branch '{}' for type '{}' doesn't exist locally",
                    name, source, bt.name
                ));
            }
        }

        // Check tag pattern variables
        if let Some(ref pattern) = bt.tag_pattern {
            let valid_vars = ["{name}", "{version}", "{date}", "{type}"];
            let pattern_lower = pattern.to_lowercase();
            
            // Check for unknown variables
            let mut pos = 0;
            while let Some(start) = pattern_lower[pos..].find('{') {
                if let Some(end) = pattern_lower[pos + start..].find('}') {
                    let var = &pattern[pos + start..pos + start + end + 1];
                    if !valid_vars.contains(&var) && var.starts_with('{') && var.ends_with('}') {
                        warnings.push(format!(
                            "[{}] Unknown variable '{}' in tag_pattern for '{}'",
                            name, var, bt.name
                        ));
                    }
                    pos = pos + start + end + 1;
                } else {
                    break;
                }
            }
        }
    }

    // Validate ticket pattern regex
    if let Some(ref pattern) = workflow.ticket_pattern {
        if regex::Regex::new(pattern).is_err() {
            errors.push(format!(
                "[{}] Invalid regex in ticket_pattern: {}",
                name, pattern
            ));
        }
    }

    // Validate rules
    if let Some(ref rules) = workflow.rules {
        if let Some(ref pattern) = rules.branch_name_pattern {
            if regex::Regex::new(pattern).is_err() {
                errors.push(format!(
                    "[{}] Invalid regex in rules.branch_name_pattern: {}",
                    name, pattern
                ));
            }
        }
    }

    // Check for ticket requirement without pattern
    for bt in &workflow.types {
        if bt.require_ticket == Some(true) && workflow.ticket_pattern.is_none() {
            errors.push(format!(
                "[{}] Type '{}' requires ticket but no ticket_pattern is defined",
                name, bt.name
            ));
        }
    }
}
