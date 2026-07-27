//! `g workflow list` — list all available workflows.

use anyhow::Result;

use crate::commands::workflow::shared::{get_active_workflow, load_workflows};
use crate::commands::Ctx;
use crate::config::workflow_presets;
use crate::ui::{muted, primary_bold, print_section};

pub fn run(_ctx: &Ctx) -> Result<()> {
    let workflows = load_workflows()?;
    let (active_name, _) = get_active_workflow().ok().unzip();

    println!();
    print_section("Available Workflows", None);
    println!();

    // Collect all workflow names (user + presets)
    let mut all_workflows: Vec<(&str, bool)> = Vec::new();

    // Add user-defined workflows
    for name in workflows.workflows.keys() {
        all_workflows.push((name.as_str(), true));
    }

    // Add presets that aren't overridden
    for preset_name in workflow_presets::preset_names() {
        if !workflows.workflows.contains_key(preset_name) {
            all_workflows.push((preset_name, false));
        }
    }

    // Sort alphabetically
    all_workflows.sort_by(|a, b| a.0.cmp(b.0));

    for (name, is_custom) in &all_workflows {
        let is_active = active_name.as_deref() == Some(*name);

        // Get workflow (from user config or preset)
        let workflow = if *is_custom {
            workflows.get(name).cloned()
        } else {
            workflow_presets::get_preset(name)
        };

        let Some(wf) = workflow else { continue };

        // Get documentation
        let docs = workflow_presets::get_docs(name);

        // Build status indicators
        let _status = if is_active { "*" } else { " " };
        let source = if *is_custom { "" } else { "(preset)" };

        // Get description
        let description = docs.map(|d| d.description).unwrap_or("Custom workflow");

        // Get branch types
        let types: Vec<&str> = wf.types.iter().map(|t| t.name.as_str()).collect();

        // Print workflow
        println!(
            "{} {} {}",
            if is_active { "*" } else { " " },
            primary_bold(name),
            muted(source)
        );
        println!("    {}", description);
        println!("    Types: {}", types.join(", "));
        println!();
    }

    // Legend
    println!("{}", muted("* = active workflow"));
    println!();
    println!("Use `g workflow info <name>` for detailed documentation.");
    println!("Use `g workflow use <name>` to switch workflows.");
    println!();

    Ok(())
}
