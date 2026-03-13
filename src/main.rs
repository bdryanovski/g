mod cli;
mod commands;
mod config;
mod github;
mod ui;

use std::error::Error;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;

use cli::{Cli, Commands, StackCommands, WorkspaceCommands};

fn main() {
    if let Err(e) = run() {
        ui::print_error(&format!("{}", e));

        // Print cause chain
        let err_ref: &dyn Error = e.as_ref();
        let mut source = err_ref.source();
        while let Some(cause) = source {
            eprintln!("  {} {}", "caused by:".bright_black(), cause.to_string().bright_black());
            source = cause.source();
        }

        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Ensure config dir and default config exist
    config::ensure_config()?;

    // Apply -C (change directory) if specified
    if let Some(dir) = &cli.directory {
        std::env::set_current_dir(dir)
            .map_err(|e| anyhow::anyhow!("Cannot change directory to '{}': {}", dir, e))?;
    }

    match cli.command {
        // ─── Workspace ────────────────────────────────────────────────────────
        Commands::Workspace(cmd) => match cmd {
            WorkspaceCommands::List => commands::workspace::list()?,
            WorkspaceCommands::Create { name, branch, description } => {
                commands::workspace::create(&name, branch.as_deref(), description.as_deref())?
            }
            WorkspaceCommands::Switch { name } => {
                commands::workspace::switch(&name)?
            }
            WorkspaceCommands::Delete { name, force } => {
                commands::workspace::delete(&name, force)?
            }
            WorkspaceCommands::Status => commands::workspace::status()?,
            WorkspaceCommands::Rename { old, new } => commands::workspace::rename(&old, &new)?,
        },

        // ─── Stack ────────────────────────────────────────────────────────────
        Commands::Stack(cmd) => match cmd {
            StackCommands::New { name } => commands::stack::new_stack(&name)?,
            StackCommands::Add { branch } => commands::stack::add_branch(&branch)?,
            StackCommands::List => commands::stack::list()?,
            StackCommands::View => commands::stack::view()?,
            StackCommands::Sync { no_interactive } => commands::stack::sync(no_interactive)?,
            StackCommands::Push { force } => commands::stack::push(force)?,
            StackCommands::Pr { open, draft } => commands::stack::create_prs(open, draft)?,
            StackCommands::Remove { branch } => commands::stack::remove_branch(&branch)?,
            StackCommands::Delete { name, branches } => {
                commands::stack::delete_stack(&name, branches)?
            }
        },

        // ─── Commit ───────────────────────────────────────────────────────────
        Commands::Commit(args) => commands::commit::commit(&args)?,

        // ─── Compare ─────────────────────────────────────────────────────────
        Commands::Compare(args) => commands::compare::compare(&args)?,

        // ─── Enhanced Git Commands ────────────────────────────────────────────
        Commands::Log(args) => commands::git::enhanced_log(&args.args)?,
        Commands::Status(args) => commands::git::enhanced_status(&args.args)?,
        Commands::Diff(args) => commands::git::enhanced_diff(&args.args)?,
        Commands::Branch(args) => commands::git::enhanced_branch(&args.args)?,
        Commands::Show(args) => commands::git::enhanced_show(&args.args)?,

        // ─── Config ───────────────────────────────────────────────────────────
        Commands::Config(args) => handle_config(args)?,

        // ─── Passthrough ─────────────────────────────────────────────────────
        Commands::Git(args) => {
            // Check aliases before passing through
            let cfg = config::load().unwrap_or_default();
            if let Some(first) = args.first() {
                if let Some(alias_target) = cfg.aliases.get(first) {
                    let mut new_args: Vec<String> =
                        alias_target.split_whitespace().map(String::from).collect();
                    new_args.extend_from_slice(&args[1..]);
                    return commands::git::passthrough(&new_args);
                }
            }
            commands::git::passthrough(&args)?
        }
    }

    Ok(())
}

fn handle_config(args: cli::ConfigArgs) -> Result<()> {
    if args.edit {
        let path = config::config_path()?;
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".into());
        std::process::Command::new(&editor)
            .arg(path.to_str().unwrap())
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to open editor '{}': {}", editor, e))?;
        return Ok(());
    }

    if args.path {
        let path = config::config_path()?;
        println!("{}", path.display());
        return Ok(());
    }

    if let Some(key) = &args.key {
        let cfg = config::load()?;
        // Very simple key lookup — just show the whole config for now
        let raw = toml::to_string_pretty(&cfg).unwrap_or_default();
        let key_lower = key.to_lowercase();
        let mut found = false;
        for line in raw.lines() {
            if line.to_lowercase().contains(&key_lower) {
                println!("{}", line.white());
                found = true;
            }
        }
        if !found {
            ui::print_warning(&format!("Key '{}' not found in config.", key));
        }
        return Ok(());
    }

    // Default: show config path and summary
    let path = config::config_path()?;
    let cfg = config::load()?;
    println!();
    println!(
        "  {} {}",
        "Config file:".bright_black(),
        path.display().to_string().cyan().underline()
    );
    println!(
        "  {} {}",
        "Default branch:".bright_black(),
        cfg.general.default_branch.green()
    );
    println!(
        "  {} {}",
        "Diff tool:".bright_black(),
        cfg.diff.tool.yellow()
    );
    println!(
        "  {} {}",
        "Aliases:".bright_black(),
        cfg.aliases.len().to_string().yellow()
    );
    println!(
        "  {} {}",
        "Commit types:".bright_black(),
        cfg.commit.types.join(", ").bright_black()
    );
    println!();
    println!(
        "  {} {}",
        "tip:".bright_black(),
        "vcli config --edit  to open in $EDITOR".bright_black()
    );
    println!();
    Ok(())
}
