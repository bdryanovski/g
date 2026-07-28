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
mod diff;
mod github;
mod hooks;
mod storage;
mod ui;

use std::error::Error;

use std::iter;
use std::sync::OnceLock;

use anyhow::{Context, Result};
use clap::{error::ErrorKind, Parser};

// Subcommand enums (WorkspaceCommands, StackCommands, DeveloperCommands,
// BranchSquashCmd) are now used inside each command module's own dispatch
// function — `main.rs` only needs the top-level `Commands` enum.
use cli::{Cli, Commands};
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
            ui::print_warning(&format!(
                "{} {}",
                ui::muted("caused by:"),
                ui::muted(&cause.to_string())
            ));
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

    // Initialise the UI theme from config.  Must happen before any output.
    // Falls back to Theme::default_dark() if config cannot be loaded.
    let cfg_for_ui = config::load().unwrap_or_default();
    let mut active_theme = ui::theme::Theme::from_config(
        &cfg_for_ui.ui.theme,
        cfg_for_ui.ui.border_style.as_deref(),
        cfg_for_ui.ui.density.as_deref(),
    );
    // When `icons = false`, or the resolved theme ended up with the ASCII border
    // style, fall back to the plain-ASCII icon set so nothing relies on Unicode.
    if !cfg_for_ui.ui.icons || active_theme.borders.style == ui::theme::BorderStyle::Ascii {
        active_theme.icons = ui::theme::Icons::ascii();
    }
    ui::theme::init(active_theme);

    // Activate inline prompt mode when configured.  The flag is checked by
    // every ui::select / ui::input / ui::confirm call and by g stage / g add.
    if cfg_for_ui.ui.prompt_mode == "inline" {
        ui::set_inline_prompts();
    }

    // Open (or create) the SQLite database.  This also runs any pending
    // migrations and performs the one-time TOML import if needed.
    let conn = db::open()?;

    // The per-invocation runtime context handed to every command. Bundles
    // the DB connection (and any future shared state) so command signatures
    // stay stable as the engine grows.
    let mut ctx = commands::Ctx::new(&conn);

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
        return commands::workspace::clone_with_workspace(&ctx, &clone_args);
    }

    // Attempt to parse using clap.  If parsing fails because the user didn't
    // choose one of our built-in subcommands, forward everything to git.
    let cli = match Cli::try_parse_from(iter::once(bin_name().to_string()).chain(raw_args.clone()))
    {
        Ok(cli) => cli,
        Err(err) => {
            // Always honour explicit --help / --version requests before any
            // passthrough logic.  Without this guard, `g --help` would fall
            // through to `should_passthrough_to_git` (which returns `true`
            // when no known subcommand is found), and git's help would be
            // shown instead of ours.
            if matches!(
                err.kind(),
                clap::error::ErrorKind::DisplayHelp
                    | clap::error::ErrorKind::DisplayVersion
                    | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
            ) {
                err.exit();
            }
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

    if cli.no_interactive {
        ui::set_no_interactive();
    }

    // Resolve repo_id best-effort — upsert so every command run registers the
    // repo and updates last_seen.  Returns None when not inside a git repo.
    let repo_id = commands::git::repo_root()
        .ok()
        .and_then(|root| storage::repos::upsert(&conn, &root).ok());
    ctx.repo_id = repo_id;

    // Record the command name and subcommand for stats.
    let (cmd_name, sub_name) = cli.command.telemetry_names();

    // Start wall-clock timer.
    let start = std::time::Instant::now();

    // Dispatch by top-level command.
    let dispatch_result: Result<()> = (|| {
        // Each command module owns its own dispatcher; `main::run` just
        // routes by top-level variant and forwards the parsed args.  Adding a
        // new subcommand variant is a one-line change in the owning module —
        // this file does not need to know about its fields.
        match cli.command {
            Commands::Workspace(cmd) => commands::workspace::dispatch(&ctx, cmd)?,
            Commands::Stack(cmd) => commands::stack::dispatch(&ctx, cmd)?,
            Commands::Developer(cmd) => commands::developer::dispatch(&ctx, cmd)?,

            Commands::Commit(args) => commands::commit::commit(&ctx, &args)?,
            Commands::Add(args) => commands::git::dispatch_add(args)?,
            Commands::Stage => commands::stage::stage()?,
            Commands::Compare(args) => commands::compare::compare(&ctx, &args)?,

            Commands::Log(args) => commands::git::enhanced_log(&args.args)?,
            Commands::Status(args) => commands::git::enhanced_status(&args.args)?,
            Commands::Diff(args) => commands::git::enhanced_diff(&ctx, &args.args)?,
            Commands::Branch(args) => commands::git::dispatch_branch(args)?,
            Commands::Show(args) => commands::git::enhanced_show(&ctx, &args.args)?,
            Commands::Push(args) => commands::git::enhanced_push(&args.args)?,
            Commands::Notes(cmd) => commands::notes::dispatch(&ctx, cmd)?,

            Commands::Stats(args) => commands::stats::stats(&ctx, &args)?,
            Commands::Config(args) => commands::config::dispatch(args)?,
            Commands::Workflow(cmd) => commands::workflow::dispatch(&ctx, cmd)?,
            Commands::Hooks(cmd) => commands::hooks::dispatch(&ctx, cmd)?,

            Commands::Completions { shell } => {
                cli::print_completions(shell);
                return Ok(());
            }

            // Unknown subcommands fall through to `git` (alias-aware).
            Commands::Git(args) => commands::git::passthrough(&args)?,
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
        "workflow",
        "commit",
        "add",
        "stage",
        "compare",
        "log",
        "stats",
        "status",
        "diff",
        "branch",
        "show",
        "config",
        "developer",
        "completions",
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
