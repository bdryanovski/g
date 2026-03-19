//! Program entry point and top-level command routing.
//!
//! Tutorial overview:
//! - `main` is the required entry point for a Rust binary crate.
//! - We delegate to `run()` so we can return a `Result` and use `?` for
//!   ergonomic error propagation.
//! - The CLI is parsed via `clap` derive macros into typed enums/structs.
//! - We then dispatch to feature modules (`commands::*`) using `match`.
//!
//! Rust concepts used here:
//! - `Result<T, E>` and the `?` operator for error propagation.
//! - Pattern matching (`match`, `if let`, `while let`) to unpack enums/Options.
//! - Trait objects (`&dyn Error`) for printing a chain of errors.
//! - Borrowing and references (`&name`, `&args`) to avoid cloning.

mod cli;
mod commands;
mod config;
mod github;
mod ui;

use std::error::Error;
use std::iter;

use anyhow::Result;
use clap::{error::ErrorKind, Parser};
use colored::Colorize;

use cli::{Cli, Commands, StackCommands, WorkspaceCommands};

/// Entry point that renders a friendly error chain and exits non-zero on failure.
fn main() {
    // `if let` unpacks the `Result` from `run()` and gives us the error case.
    if let Err(e) = run() {
        ui::print_error(&format!("{}", e));

        // Print cause chain for better debugging.
        // `anyhow::Error` can carry a source chain; we walk it via `std::error::Error`.
        let err_ref: &dyn Error = e.as_ref();
        let mut source = err_ref.source();
        // `while let` keeps looping while `source` is `Some(...)`.
        while let Some(cause) = source {
            eprintln!(
                "  {} {}",
                "caused by:".bright_black(),
                cause.to_string().bright_black()
            );
            source = cause.source();
        }

        std::process::exit(1);
    }
}

/// Parse CLI arguments, ensure config exists, then dispatch commands.
fn run() -> Result<()> {
    // Capture raw args so we can fallback to a pure git passthrough when
    // users supply no known subcommand (e.g., `g -m "msg" -A`).
    let raw_args: Vec<String> = std::env::args().skip(1).collect();

    // Attempt to parse using clap; if parsing fails because the user didn't
    // choose one of our built-in subcommands, forward everything to git.
    let cli = match Cli::try_parse_from(iter::once("g".to_string()).chain(raw_args.clone())) {
        Ok(cli) => cli,
        Err(err) => {
            if should_passthrough_to_git(&raw_args) || should_passthrough_on_parse_error(&err) {
                return commands::git::passthrough(&raw_args);
            }
            // Preserve clap's nice error output for genuine CLI mistakes.
            err.exit();
        }
    };

    // Ensure config dir and default config exist.
    // `?` means: if this returns `Err`, bubble it up to `main`.
    config::ensure_config()?;

    // Apply -C (change directory) if specified.
    // `Option<T>` is Rust's "maybe" type; `if let Some(dir)` extracts the value.
    if let Some(dir) = &cli.directory {
        std::env::set_current_dir(dir)
            .map_err(|e| anyhow::anyhow!("Cannot change directory to '{}': {}", dir, e))?;
    }

    // Dispatch by top-level command.
    match cli.command {
        // ─── Workspace ────────────────────────────────────────────────────────
        Commands::Workspace(cmd) => match cmd {
            WorkspaceCommands::List => commands::workspace::list()?,
            WorkspaceCommands::Create {
                name,
                branch,
                description,
            } => {
                // `as_deref()` turns `Option<String>` into `Option<&str>` without cloning.
                commands::workspace::create(&name, branch.as_deref(), description.as_deref())?
            }
            WorkspaceCommands::Switch { name } => commands::workspace::switch(&name)?,
            WorkspaceCommands::Delete { name, force } => commands::workspace::delete(&name, force)?,
            WorkspaceCommands::Status => commands::workspace::status()?,
            WorkspaceCommands::Rename { old, new } => commands::workspace::rename(&old, &new)?,
        },

        // ─── Stack ────────────────────────────────────────────────────────────
        Commands::Stack(cmd) => match cmd {
            StackCommands::New { name } => commands::stack::new_stack(&name)?,
            StackCommands::Add { branch } => commands::stack::add_branch(&branch)?,
            StackCommands::List => commands::stack::list()?,
            StackCommands::View => commands::stack::view()?,
            StackCommands::Details => commands::stack::details()?,
            StackCommands::Switch { name } => commands::stack::switch_stack(&name)?,
            StackCommands::Absorb => commands::stack::absorb()?,
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
            // Check aliases before passing through.
            let cfg = config::load().unwrap_or_default();
            if let Some(first) = args.first() {
                if let Some(alias_target) = cfg.aliases.get(first) {
                    // Split the alias into words and append the original args.
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

/// Decide if we should skip our CLI handling and forward args straight to git.
///
/// Rules:
/// - If no args were provided, keep clap's help output (return false).
/// - If the first non-global token isn't one of our built-in subcommands,
///   treat it as a raw git invocation and passthrough (return true).
fn should_passthrough_to_git(raw_args: &[String]) -> bool {
    if raw_args.is_empty() {
        return false;
    }

    // Built-in commands we want to keep handling ourselves.
    const KNOWN: &[&str] = &[
        "workspace",
        "stack",
        "commit",
        "compare",
        "log",
        "status",
        "diff",
        "branch",
        "show",
        "config",
    ];

    match first_non_global_token(raw_args) {
        Some(cmd) => !KNOWN.contains(&cmd.as_str()),
        None => true,
    }
}

/// If clap rejects arguments because of an unknown flag/arg, prefer to let git
/// handle it instead of blocking the user. This keeps `g commit -s -S` working
/// as a direct git passthrough.
fn should_passthrough_on_parse_error(err: &clap::Error) -> bool {
    matches!(err.kind(), ErrorKind::UnknownArgument)
}

/// Find the first arg that isn't a global flag we handle (-C/-c) or another
/// flag (starts with '-') to infer the intended git subcommand.
fn first_non_global_token(raw_args: &[String]) -> Option<String> {
    let mut iter = raw_args.iter().peekable();
    while let Some(arg) = iter.next() {
        // Respect end-of-options marker.
        if arg == "--" {
            return iter.next().cloned();
        }

        // Skip our global directory/config options and their values.
        if arg == "-C" {
            iter.next();
            continue;
        }
        if arg.starts_with("-C") && arg.len() > 2 {
            continue;
        }
        if arg == "-c" {
            iter.next();
            continue;
        }
        if arg.starts_with("-c") && arg.len() > 2 {
            continue;
        }

        // Any other flag: skip.
        if arg.starts_with('-') {
            continue;
        }

        return Some(arg.clone());
    }
    None
}

/// Handle `g config` subcommands and default config display.
fn handle_config(args: cli::ConfigArgs) -> Result<()> {
    if args.edit {
        let path = config::config_path()?;
        // Read environment variable with a fallback.
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".into());
        // Spawn the editor process and wait for it to exit.
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
        "g config --edit  to open in $EDITOR".bright_black()
    );
    println!();
    Ok(())
}

// TODO(main): Consider a structured error reporter (e.g. color-eyre) for richer backtraces.
// TODO(main): Add tests for command dispatch (e.g., `Cli::try_parse_from` with args).
