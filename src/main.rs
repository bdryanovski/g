//! Program entry point and top-level command routing.
//!
//! ## Tutorial overview
//!
//! - `main` is the required entry point for a Rust binary crate.
//! - We delegate to [`run`] so we can return a `Result` and use `?` for
//!   ergonomic error propagation.
//! - The CLI is parsed via `clap` derive macros into typed enums/structs.
//! - We then dispatch to feature modules (`commands::*`) using `match`.
//!
//! ## Rust concepts used here
//!
//! - `Result<T, E>` and the `?` operator for error propagation.
//! - Pattern matching (`match`, `if let`, `while let`) to unpack enums/Options.
//! - Trait objects (`&dyn Error`) for printing a chain of errors.
//! - Borrowing and references (`&name`, `&args`) to avoid cloning.

// ─── Crate-level lint configuration ─────────────────────────────────────────
//
// These attributes configure the Rust compiler and Clippy lints for the whole
// crate.  They follow the priority order from the rust-skills guide:
//   CRITICAL → correctness (real bugs), suspicious (likely bugs)
//   HIGH     → style, complexity, performance
//
// `deny` turns a lint category into a hard error; `warn` shows it but lets the
// build succeed.  We use `warn` for everything here so learners can still build
// while they address the notices.
#![warn(clippy::correctness)]
#![warn(clippy::suspicious)]
#![warn(clippy::style)]
#![warn(clippy::complexity)]
#![warn(clippy::perf)]
// Require `///` documentation on every public item.  This enforces the
// `doc-all-public` rule and helps readers learn by reading the code.
#![warn(missing_docs)]

mod cli;
mod commands;
mod config;
mod github;
mod storage;
mod ui;

use std::error::Error;
use std::iter;
use std::sync::OnceLock;

use anyhow::{Context, Result};
use clap::{error::ErrorKind, Parser};
use colored::Colorize;

use cli::{BranchSquashCmd, Cli, Commands, DeveloperCommands, StackCommands, WorkspaceCommands};
use storage::{db, stats};

// ─── Application identity ─────────────────────────────────────────────────────

/// Stable application identifier used for storage directories, plugin naming,
/// and any other place that needs to remain constant even if the binary is renamed.
///
/// **Why this exists separately from [`bin_name`]:**
/// - [`bin_name`] returns the *runtime* name of the binary (e.g. `"git-stack"`
///   if someone renames or symlinks the executable).  It is used in user-facing
///   messages so `--help` text and error hints always show the correct command.
/// - `APP_ID` is the *stable identity* baked into this build.  The config
///   directory (`~/.config/g/`), plugin prefix (`g-*`), and `Cargo.toml`
///   package name all use this constant.  Renaming the binary does **not**
///   move your config or break plugin discovery — only a deliberate code change
///   to this constant does.
pub(crate) const APP_ID: &str = "g";

// `OnceLock<T>` is Rust's built-in lazy, thread-safe, write-once cell.
// It initialises on the first call and caches the result for the rest of
// the process lifetime — no mutex overhead on subsequent reads.
static BIN_NAME: OnceLock<String> = OnceLock::new();

/// Returns the name of the currently running binary.
///
/// On the first call this reads `std::env::args().next()`, strips the directory
/// path (so `/usr/local/bin/git-stack` becomes `"git-stack"`), and caches the
/// result.  All subsequent calls return the cached `&'static str` with zero cost.
///
/// Falls back to [`APP_ID`] if the name cannot be determined (e.g. when the
/// binary is invoked in a way that provides no argv\[0\]).
///
/// # Why `&'static str`?
///
/// The value is stored in a `static`, which means it lives for the entire
/// program lifetime.  Returning `&'static str` lets every caller use the name
/// without cloning or reference counting.
pub(crate) fn bin_name() -> &'static str {
    BIN_NAME.get_or_init(|| {
        std::env::args()
            .next()
            .as_deref()
            // Extract just the filename: "/usr/local/bin/git-stack" → "git-stack"
            .and_then(|s| std::path::Path::new(s).file_name())
            .and_then(|n| n.to_str())
            .unwrap_or(APP_ID)
            .to_string()
    })
}

/// Entry point: renders a friendly error chain and exits non-zero on failure.
///
/// `main` itself cannot return `Result` with a custom formatter, so we call
/// [`run`] and handle any error here with pretty printing.
fn main() {
    // `if let` unpacks the `Result` from `run()` and gives us the error case.
    if let Err(e) = run() {
        ui::print_error(&format!("{}", e));

        // Print the full cause chain for better debugging.
        // `anyhow::Error` can carry a source chain; we walk it via
        // `std::error::Error::source`.
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

/// Parse CLI arguments, ensure config exists, then dispatch to the right command.
///
/// # Errors
///
/// Returns an error if:
/// - The config directory cannot be created or the default config cannot be written.
/// - The `-C` directory does not exist or cannot be entered.
/// - Any subcommand returns an error.
fn run() -> Result<()> {
    // Capture raw args so we can fall back to a pure git passthrough when
    // users supply no known subcommand (e.g., `g -m "msg" -A`).
    let raw_args: Vec<String> = std::env::args().skip(1).collect();

    // Ensure the config directory and default config file exist before anything
    // else — db::open() needs the directory to already exist for config.toml.
    config::ensure_config()?;

    // Open (or create) the SQLite database.  This also runs any pending
    // migrations and performs the one-time TOML import if needed.
    let conn = db::open()?;

    // Intercept `g clone --workspace` before clap or git passthrough.
    // Strip the `--workspace` flag and delegate to the workspace handler.
    if raw_args.first().map(|s| s.as_str()) == Some("clone")
        && raw_args.iter().any(|a| a == "--workspace")
    {
        let clone_args: Vec<String> = raw_args
            .iter()
            .filter(|a| a.as_str() != "--workspace")
            .cloned()
            .collect();
        return commands::workspace::clone_with_workspace(&conn, &clone_args);
    }

    // Attempt to parse using clap.  If parsing fails because the user didn't
    // choose one of our built-in subcommands, forward everything to git.
    let cli = match Cli::try_parse_from(iter::once(bin_name().to_string()).chain(raw_args.clone()))
    {
        Ok(cli) => cli,
        Err(err) => {
            if should_passthrough_to_git(&raw_args) || should_passthrough_on_parse_error(&err) {
                return commands::git::passthrough(&raw_args);
            }
            // Preserve clap's nice error output for genuine CLI mistakes.
            err.exit();
        }
    };

    // Apply -C (change directory) if specified.
    // `Option<T>` is Rust's "maybe" type; `if let Some(dir)` extracts the value.
    if let Some(dir) = &cli.directory {
        std::env::set_current_dir(dir)
            .with_context(|| format!("Cannot change directory to '{}'", dir))?;
    }

    let dry_run = cli.dry_run;
    if dry_run {
        commands::git::set_dry_run(true);
        commands::git::dry_run_banner();
    }

    // Resolve repo_id best-effort — upsert so every command run registers the
    // repo and updates last_seen.  Returns None when not inside a git repo.
    let repo_id = commands::git::repo_root()
        .ok()
        .and_then(|root| storage::repos::upsert(&conn, &root).ok());

    // Record the command name and subcommand for stats.
    let (cmd_name, sub_name) = command_names(&cli.command);

    // Start wall-clock timer.
    let start = std::time::Instant::now();

    // Dispatch by top-level command.
    let dispatch_result: Result<()> = (|| {
        match cli.command {
            // ─── Workspace ────────────────────────────────────────────────────────
            Commands::Workspace(cmd) => match cmd {
                WorkspaceCommands::Init => commands::workspace::init(&conn)?,
                WorkspaceCommands::List => commands::workspace::list(&conn)?,
                WorkspaceCommands::Create {
                    name,
                    branch,
                    start_point,
                    description,
                    copy,
                } => {
                    // `as_deref()` turns `Option<String>` into `Option<&str>` without cloning.
                    commands::workspace::create(
                        &conn,
                        &name,
                        branch.as_deref(),
                        start_point.as_deref(),
                        description.as_deref(),
                        copy,
                    )?
                }
                WorkspaceCommands::Switch { name } => {
                    commands::workspace::switch(&conn, name.as_deref())?
                }
                WorkspaceCommands::Delete { name, force } => {
                    commands::workspace::delete(&conn, &name, force)?
                }
                WorkspaceCommands::Status => commands::workspace::status(&conn)?,
                WorkspaceCommands::Rename { old, new } => {
                    commands::workspace::rename(&conn, &old, &new)?
                }
            },

            // ─── Stack ────────────────────────────────────────────────────────────
            Commands::Stack(cmd) => match cmd {
                StackCommands::New { name } => commands::stack::new_stack(&conn, &name)?,
                StackCommands::Add { branch } => commands::stack::add_branch(&conn, &branch)?,
                StackCommands::List => commands::stack::list(&conn)?,
                StackCommands::View => commands::stack::view(&conn)?,
                StackCommands::Details => commands::stack::details(&conn)?,
                StackCommands::Switch { name } => commands::stack::switch_stack(&conn, &name)?,
                StackCommands::Absorb => commands::stack::absorb(&conn)?,
                StackCommands::Squash {
                    message,
                    no_interactive,
                } => commands::stack::squash(&conn, message.as_deref(), no_interactive)?,
                StackCommands::Fold {
                    keep,
                    no_interactive,
                } => commands::stack::fold(&conn, keep, no_interactive)?,
                StackCommands::Sync { no_interactive } => {
                    commands::stack::sync(&conn, no_interactive)?
                }
                StackCommands::Push { force } => commands::stack::push(&conn, force)?,
                StackCommands::Pr { open, draft } => {
                    commands::stack::create_prs(&conn, open, draft)?
                }
                StackCommands::Remove { branch } => commands::stack::remove_branch(&conn, &branch)?,
                StackCommands::Delete { name, branches } => {
                    commands::stack::delete_stack(&conn, &name, branches)?
                }
                StackCommands::Up => commands::stack::move_up(&conn)?,
                StackCommands::Down => commands::stack::move_down(&conn)?,
            },

            // ─── Commit ───────────────────────────────────────────────────────────
            Commands::Commit(args) => commands::commit::commit(&conn, &args)?,

            // ─── Compare ─────────────────────────────────────────────────────────
            Commands::Compare(args) => commands::compare::compare(&args)?,

            // ─── Enhanced Git Commands ────────────────────────────────────────────
            Commands::Log(args) => commands::git::enhanced_log(&args.args)?,
            Commands::Status(args) => commands::git::enhanced_status(&args.args)?,
            Commands::Diff(args) => commands::git::enhanced_diff(&args.args)?,
            Commands::Branch(args) => {
                if let Some(BranchSquashCmd::Squash { message, base }) = args.cmd {
                    commands::git::branch_squash(message.as_deref(), base.as_deref())?;
                } else {
                    commands::git::enhanced_branch(&args.rest)?;
                }
            }
            Commands::Show(args) => commands::git::enhanced_show(&args.args)?,

            // ─── Config ───────────────────────────────────────────────────────────
            Commands::Config(args) => handle_config(args)?,

            // ─── Developer ────────────────────────────────────────────────────────
            Commands::Developer(cmd) => match cmd {
                DeveloperCommands::Db { path } => commands::developer::db(path)?,
                DeveloperCommands::Repos => commands::developer::repos(&conn)?,
            },

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

        if dry_run {
            commands::git::dry_run_footer();
        }

        Ok(())
    })();

    // Record the command run — best-effort, never fails the CLI.
    let duration_ms = start.elapsed().as_millis() as u64;
    let (exit_code, error_msg) = match &dispatch_result {
        Ok(_) => (0i32, None),
        Err(e) => (1i32, Some(e.to_string())),
    };
    stats::record_command(
        &conn,
        cmd_name,
        sub_name,
        repo_id,
        Some(duration_ms),
        exit_code,
        error_msg.as_deref(),
    )
    .ok();

    dispatch_result
}

/// Returns `true` if we should skip our CLI handling and forward args straight to git.
///
/// Rules:
/// - If no args were provided, keep clap's help output (return false).
/// - If the first non-global token isn't one of our built-in subcommands,
///   treat it as a raw git invocation and passthrough (return true).
fn should_passthrough_to_git(raw_args: &[String]) -> bool {
    if raw_args.is_empty() {
        return false;
    }

    // Built-in commands we handle ourselves; everything else goes to git.
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
        "developer",
    ];

    match first_non_global_token(raw_args) {
        Some(cmd) => !KNOWN.contains(&cmd.as_str()),
        None => true,
    }
}

/// Returns `true` if clap rejected arguments due to an unknown flag/arg.
///
/// In that case we prefer to let git handle it instead of showing clap's error.
/// This keeps `g commit -s -S` working as a direct git passthrough.
fn should_passthrough_on_parse_error(err: &clap::Error) -> bool {
    matches!(err.kind(), ErrorKind::UnknownArgument)
}

/// Finds the first arg that is not a global flag (`-C`/`-c`) or any other flag
/// starting with `-`, which is used to infer the intended git subcommand.
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

/// Handles `g config` subcommands: edit, path, key lookup, and default summary.
///
/// # Errors
///
/// Returns an error if:
/// - The config path cannot be determined.
/// - The config file cannot be loaded or serialized.
/// - The editor process cannot be spawned.
fn handle_config(args: cli::ConfigArgs) -> Result<()> {
    if args.edit {
        let path = config::config_path()?;
        // Read `$EDITOR` with a fallback to `vim`.
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".into());
        // `path.to_str()` returns `None` if the path contains non-UTF-8 bytes.
        let path_str = path
            .to_str()
            .context("Config path contains non-UTF-8 characters")?;
        // Spawn the editor process and wait for it to exit.
        std::process::Command::new(&editor)
            .arg(path_str)
            .status()
            .with_context(|| format!("Failed to open editor '{}'", editor))?;
        return Ok(());
    }

    if args.path {
        let path = config::config_path()?;
        println!("{}", path.display());
        return Ok(());
    }

    if let Some(key) = &args.key {
        let cfg = config::load()?;
        // Serialize the whole config to TOML and filter lines that match the key.
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

    // Default: show config path and a human-readable summary.
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
        format!("{} config --edit  to open in $EDITOR", bin_name()).bright_black()
    );
    println!();
    Ok(())
}

/// Return the top-level command name and optional subcommand name for stats recording.
///
/// Examples:
/// - `g commit`            → `("commit", None)`
/// - `g workspace create`  → `("workspace", Some("create"))`
/// - `g stack pr`          → `("stack", Some("pr"))`
/// - `g log`               → `("log", None)`
fn command_names(cmd: &Commands) -> (&'static str, Option<&'static str>) {
    match cmd {
        Commands::Workspace(sub) => {
            let sub_name = match sub {
                WorkspaceCommands::Init => "init",
                WorkspaceCommands::List => "list",
                WorkspaceCommands::Create { .. } => "create",
                WorkspaceCommands::Switch { .. } => "switch",
                WorkspaceCommands::Delete { .. } => "delete",
                WorkspaceCommands::Status => "status",
                WorkspaceCommands::Rename { .. } => "rename",
            };
            ("workspace", Some(sub_name))
        }
        Commands::Stack(sub) => {
            let sub_name = match sub {
                StackCommands::New { .. } => "new",
                StackCommands::Add { .. } => "add",
                StackCommands::List => "list",
                StackCommands::View => "view",
                StackCommands::Details => "details",
                StackCommands::Switch { .. } => "switch",
                StackCommands::Absorb => "absorb",
                StackCommands::Squash { .. } => "squash",
                StackCommands::Fold { .. } => "fold",
                StackCommands::Sync { .. } => "sync",
                StackCommands::Push { .. } => "push",
                StackCommands::Pr { .. } => "pr",
                StackCommands::Remove { .. } => "remove",
                StackCommands::Delete { .. } => "delete",
                StackCommands::Up => "up",
                StackCommands::Down => "down",
            };
            ("stack", Some(sub_name))
        }
        Commands::Commit(_) => ("commit", None),
        Commands::Compare(_) => ("compare", None),
        Commands::Log(_) => ("log", None),
        Commands::Status(_) => ("status", None),
        Commands::Diff(_) => ("diff", None),
        Commands::Branch(_) => ("branch", None),
        Commands::Show(_) => ("show", None),
        Commands::Config(_) => ("config", None),
        Commands::Developer(sub) => {
            let sub_name = match sub {
                DeveloperCommands::Db { .. } => "db",
                DeveloperCommands::Repos => "repos",
            };
            ("developer", Some(sub_name))
        }
        Commands::Git(args) => {
            // For passthrough commands, record the first arg as the subcommand.
            // We return a static "git" here; the subcommand is dynamic so we
            // record it as None (dynamic strings can't be &'static str).
            let _ = args;
            ("git", None)
        }
    }
}
