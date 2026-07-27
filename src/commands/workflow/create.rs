//! `g workflow create` — create a new workflow interactively.

use anyhow::{bail, Result};

use crate::cli::workflow::CreateArgs;
use crate::commands::workflow::shared::load_workflows;
use crate::commands::Ctx;
use crate::config::workflow::*;
use crate::config::workflow_presets;
use crate::config::{self};
use crate::ui::{self, confirm, input, multi_select, print_section, select, SelectOption};

pub fn run(_ctx: &Ctx, args: CreateArgs) -> Result<()> {
    // Check if name already exists
    if let Some(ref name) = args.name {
        let workflows = load_workflows()?;
        if workflows.get(name).is_some() || workflow_presets::get_preset(name).is_some() {
            bail!(
                "Workflow '{}' already exists. Use `g workflow edit {}` to modify it.",
                name,
                name
            );
        }
    }

    // Determine starting point
    let base_workflow = if let Some(ref preset) = args.from {
        workflow_presets::get_preset(preset).ok_or_else(|| {
            anyhow::anyhow!(
                "Preset '{}' not found. Available: {}",
                preset,
                workflow_presets::preset_names().join(", ")
            )
        })?
    } else if args.no_interactive {
        Workflow::default()
    } else {
        // Interactive: ask which preset to start from
        select_base_workflow()?
    };

    // Get workflow name
    let name = if let Some(n) = args.name {
        n
    } else if args.no_interactive {
        bail!("Workflow name is required in non-interactive mode. Use: g workflow create <name>");
    } else {
        input("Workflow name", None).ok_or_else(|| anyhow::anyhow!("Name is required"))?
    };

    if args.no_interactive {
        // Save defaults directly
        save_workflow(&name, base_workflow, args.local)?;
        ui::print_success(&format!("Created workflow '{}'", name));
        return Ok(());
    }

    // Interactive workflow builder
    println!();
    print_section(&format!("Creating workflow: {}", name), None);
    println!();

    let workflow = build_workflow_interactive(base_workflow)?;

    // Preview
    println!();
    print_section("Preview", None);
    println!();

    let toml_preview = toml::to_string_pretty(&workflow)?;
    for line in toml_preview.lines().take(30) {
        println!("  {}", line);
    }
    if toml_preview.lines().count() > 30 {
        println!("  ...");
    }

    println!();

    // Confirm save
    if !confirm("Save this workflow?", true) {
        ui::print_info("Cancelled.");
        return Ok(());
    }

    // Edit in $EDITOR?
    if confirm("Edit in $EDITOR before saving?", false) {
        let edited = edit_in_editor(&toml_preview)?;
        let workflow: Workflow = toml::from_str(&edited)?;
        save_workflow(&name, workflow, args.local)?;
    } else {
        save_workflow(&name, workflow, args.local)?;
    }

    ui::print_success(&format!("Created workflow '{}'", name));
    println!();
    println!("Next steps:");
    println!("  g workflow use {}     # Activate this workflow", name);
    println!("  g workflow info {}    # View details", name);
    println!();

    Ok(())
}

/// Let user pick a base workflow or start from scratch.
fn select_base_workflow() -> Result<Workflow> {
    let preset_names = workflow_presets::preset_names();

    let mut options: Vec<SelectOption> = vec![SelectOption::with_description(
        "Start from scratch",
        "Empty workflow with no branch types",
    )];

    for name in &preset_names {
        if workflow_presets::get_preset(name).is_some() {
            let desc = workflow_presets::get_docs(name)
                .map(|d| d.description)
                .unwrap_or("");
            options.push(SelectOption::with_description(name.to_string(), desc));
        }
    }

    let idx = select("Start from", &options).unwrap_or(0);

    if idx == 0 {
        Ok(Workflow::default())
    } else {
        let preset_name = &preset_names[idx - 1];
        workflow_presets::get_preset(preset_name).ok_or_else(|| anyhow::anyhow!("Preset not found"))
    }
}

fn build_workflow_interactive(mut workflow: Workflow) -> Result<Workflow> {
    // Main branch
    let main_branch = input("Main branch", Some(&workflow.main_branch))
        .unwrap_or_else(|| workflow.main_branch.clone());
    workflow.main_branch = main_branch;

    // Develop branch
    let use_develop = confirm(
        "Use a separate development branch?",
        workflow.develop_branch.is_some(),
    );
    if use_develop {
        let default = workflow.develop_branch.as_deref().unwrap_or("develop");
        workflow.develop_branch = input("Development branch", Some(default));
    } else {
        workflow.develop_branch = None;
    }

    // Branch types
    println!();
    print_section("Branch Types", None);
    println!();

    if !workflow.types.is_empty() {
        println!("Current types:");
        for bt in &workflow.types {
            println!(
                "  • {} ({}) → {}",
                bt.name,
                bt.prefix,
                bt.target
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| bt.source.clone())
            );
        }
        println!();
    }

    loop {
        let actions = if workflow.types.is_empty() {
            vec![
                SelectOption::new("Add new branch type"),
                SelectOption::new("Done"),
            ]
        } else {
            vec![
                SelectOption::new("Add new branch type"),
                SelectOption::new("Edit existing type"),
                SelectOption::new("Remove type"),
                SelectOption::new("Done"),
            ]
        };

        let action = select("Branch types", &actions).unwrap_or(actions.len() - 1);

        if workflow.types.is_empty() {
            match action {
                0 => {
                    if let Some(bt) = create_branch_type_interactive()? {
                        workflow.types.push(bt);
                    }
                }
                _ => break,
            }
        } else {
            match action {
                0 => {
                    if let Some(bt) = create_branch_type_interactive()? {
                        workflow.types.push(bt);
                    }
                }
                1 => {
                    let type_options: Vec<SelectOption> = workflow
                        .types
                        .iter()
                        .map(|t| SelectOption::with_description(&t.name, &t.prefix))
                        .collect();
                    if let Some(idx) = select("Select type to edit", &type_options) {
                        if let Some(bt) = edit_branch_type_interactive(&workflow.types[idx])? {
                            workflow.types[idx] = bt;
                        }
                    }
                }
                2 => {
                    let type_options: Vec<SelectOption> = workflow
                        .types
                        .iter()
                        .map(|t| SelectOption::with_description(&t.name, &t.prefix))
                        .collect();
                    if let Some(idx) = select("Select type to remove", &type_options) {
                        let name = workflow.types[idx].name.clone();
                        workflow.types.remove(idx);
                        ui::print_info(&format!("Removed '{}'", name));
                    }
                }
                _ => break,
            }
        }
    }

    // Rules
    println!();
    print_section("Rules", None);
    println!();

    let rule_options = vec![
        SelectOption::with_description(
            "Require clean working tree",
            "Block operations if there are uncommitted changes",
        ),
        SelectOption::with_description(
            "Require source branch up-to-date",
            "Block if source has new commits",
        ),
    ];

    let mut preselected = vec![];
    if let Some(ref rules) = workflow.rules {
        if rules.require_clean_tree.unwrap_or(false) {
            preselected.push(0);
        }
        if rules.require_up_to_date.unwrap_or(false) {
            preselected.push(1);
        }
    }

    let selected = multi_select("Select rules to enforce", &rule_options);

    let require_clean = selected.contains(&0);
    let require_up_to_date = selected.contains(&1);

    if require_clean || require_up_to_date {
        workflow.rules = Some(WorkflowRules {
            require_clean_tree: Some(require_clean),
            require_up_to_date: Some(require_up_to_date),
            ..Default::default()
        });
    } else {
        workflow.rules = None;
    }

    Ok(workflow)
}

fn create_branch_type_interactive() -> Result<Option<BranchType>> {
    println!();
    ui::print_info("Creating new branch type...");
    println!();

    let name = match input("Type name (e.g., feature, hotfix)", None) {
        Some(n) if !n.is_empty() => n,
        _ => {
            ui::print_warning("Cancelled.");
            return Ok(None);
        }
    };

    let default_prefix = format!("{}/", name);
    let prefix = input("Branch prefix", Some(&default_prefix)).unwrap_or(default_prefix);

    let source =
        input("Source branch (create from)", Some("main")).unwrap_or_else(|| "main".into());

    let target = input("Target branch (merge to)", Some(&source)).unwrap_or_else(|| source.clone());

    // Merge strategy
    let strategy_options = vec![
        SelectOption::with_description("squash", "Combine all commits into one"),
        SelectOption::with_description("merge", "Create a merge commit"),
        SelectOption::with_description("rebase", "Rebase commits onto target"),
        SelectOption::with_description("fast-forward", "Fast-forward only (fails if not possible)"),
        SelectOption::with_description("cherry-pick", "Cherry-pick specific commits"),
    ];

    let strategy_idx = select("Merge strategy", &strategy_options).unwrap_or(0);
    let merge_strategy = match strategy_idx {
        0 => MergeStrategy::Squash,
        1 => MergeStrategy::Merge,
        2 => MergeStrategy::Rebase,
        3 => MergeStrategy::FfOnly,
        _ => MergeStrategy::CherryPick,
    };

    // Options
    let option_choices = vec![
        SelectOption::with_description("Delete branch after merge", "Clean up merged branches"),
        SelectOption::with_description("Require PR", "No direct merge allowed"),
        SelectOption::with_description("Tag on finish", "Create a tag when finishing"),
    ];

    let selected_options = multi_select("Options", &option_choices);

    let delete_after = selected_options.contains(&0);
    let require_pr = selected_options.contains(&1);
    let tag_on_finish = selected_options.contains(&2);

    Ok(Some(BranchType {
        name,
        prefix,
        source,
        target: Some(BranchTarget::Single(target)),
        merge_strategy,
        delete_after_merge: Some(delete_after),
        require_pr: Some(require_pr),
        tag_on_finish: if tag_on_finish { Some(true) } else { None },
        ..Default::default()
    }))
}

fn edit_branch_type_interactive(bt: &BranchType) -> Result<Option<BranchType>> {
    println!();
    ui::print_info(&format!("Editing type '{}'...", bt.name));
    println!();

    let name = input("Type name", Some(&bt.name)).unwrap_or_else(|| bt.name.clone());
    let prefix = input("Branch prefix", Some(&bt.prefix)).unwrap_or_else(|| bt.prefix.clone());
    let source = input("Source branch", Some(&bt.source)).unwrap_or_else(|| bt.source.clone());

    let current_target = bt
        .target
        .as_ref()
        .map(|t| t.to_string())
        .unwrap_or_default();
    let target =
        input("Target branch", Some(&current_target)).unwrap_or_else(|| current_target.clone());

    // Merge strategy
    let strategy_options = vec![
        SelectOption::with_description("squash", "Combine all commits into one"),
        SelectOption::with_description("merge", "Create a merge commit"),
        SelectOption::with_description("rebase", "Rebase commits onto target"),
        SelectOption::with_description("fast-forward", "Fast-forward only"),
        SelectOption::with_description("cherry-pick", "Cherry-pick specific commits"),
    ];

    let current_idx = match bt.merge_strategy {
        MergeStrategy::Squash => 0,
        MergeStrategy::Merge => 1,
        MergeStrategy::Rebase => 2,
        MergeStrategy::FfOnly => 3,
        MergeStrategy::CherryPick => 4,
    };

    ui::print_info(&format!(
        "Current strategy: {}",
        strategy_options[current_idx].label
    ));
    let strategy_idx = select("Merge strategy", &strategy_options).unwrap_or(current_idx);
    let merge_strategy = match strategy_idx {
        0 => MergeStrategy::Squash,
        1 => MergeStrategy::Merge,
        2 => MergeStrategy::Rebase,
        3 => MergeStrategy::FfOnly,
        _ => MergeStrategy::CherryPick,
    };

    Ok(Some(BranchType {
        name,
        prefix,
        source,
        target: Some(BranchTarget::Single(target)),
        merge_strategy,
        delete_after_merge: bt.delete_after_merge,
        require_pr: bt.require_pr,
        tag_on_finish: bt.tag_on_finish,
        ..Default::default()
    }))
}

fn save_workflow(name: &str, workflow: Workflow, local: bool) -> Result<()> {
    if local {
        // Save to repo-local config
        let mut workflows = match config::repo_workflow_path() {
            Ok(path) if path.exists() => {
                let raw = std::fs::read_to_string(&path)?;
                toml::from_str(&raw)?
            }
            _ => WorkflowsConfig::default(),
        };

        workflows.workflows.insert(name.to_string(), workflow);
        config::save_repo_workflows(&workflows)?;
        ui::print_info("Saved to .g/workflow.toml");
    } else {
        // Save to global config
        let mut cfg = config::load()?;
        cfg.workflows.workflows.insert(name.to_string(), workflow);
        config::save(&cfg)?;
        ui::print_info("Saved to global config");
    }
    Ok(())
}

fn edit_in_editor(content: &str) -> Result<String> {
    use std::env;
    use std::fs;

    let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
    let temp_file = std::env::temp_dir().join("g-workflow-edit.toml");

    fs::write(&temp_file, content)?;

    let status = std::process::Command::new(&editor)
        .arg(&temp_file)
        .status()?;

    if !status.success() {
        bail!("Editor exited with non-zero status");
    }

    let edited = fs::read_to_string(&temp_file)?;
    fs::remove_file(&temp_file)?;

    Ok(edited)
}
