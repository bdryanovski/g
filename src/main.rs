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
use std::io::IsTerminal;
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
    let ctx = commands::Ctx::new(&conn);

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
            Commands::Compare(args) => commands::compare::compare(&args)?,

            Commands::Log(args) => commands::git::enhanced_log(&args.args)?,
            Commands::Status(args) => commands::git::enhanced_status(&args.args)?,
            Commands::Diff(args) => commands::git::enhanced_diff(&args.args)?,
            Commands::Branch(args) => commands::git::dispatch_branch(args)?,
            Commands::Show(args) => commands::git::enhanced_show(&args.args)?,

            Commands::Stats(args) => commands::stats::stats(&ctx, &args)?,
            Commands::Config(args) => handle_config(args)?,

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
        ui::print_line(&path.display().to_string());
        return Ok(());
    }

    if args.themes {
        return handle_themes();
    }

    if args.new_theme {
        return create_theme_wizard(None);
    }

    // ── Subcommand: `g config set <key> <value>` ────────────────────────────
    if let Some(cli::ConfigCmd::Set { key, value }) = &args.cmd {
        return handle_config_set(key, value);
    }

    // ── --get <key>: print exact current value (scripting-friendly) ────────
    if let Some(key) = &args.get {
        return handle_config_get(key);
    }

    // ── --list: every editable scalar with its current value + help ───────
    if args.list {
        return handle_config_list();
    }

    // ── --menu: interactive picker over the schema ────────────────────────
    if args.menu {
        return handle_config_menu();
    }

    if let Some(key) = &args.key {
        let cfg = config::load()?;
        // Serialize the whole config to TOML and filter lines that match the key.
        let raw = toml::to_string_pretty(&cfg).unwrap_or_default();
        let key_lower = key.to_lowercase();
        let mut found = false;
        for line in raw.lines() {
            if line.to_lowercase().contains(&key_lower) {
                ui::print_line(&ui::paint_text(line));
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
    let db_path = config::db_path()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    ui::print_blank();
    ui::print_fieldset("Configuration");
    ui::print_blank();
    ui::print_key_value_pairs(&[
        (
            "Config file",
            ui::link_primary_bold(&path.display().to_string()),
        ),
        ("Database", ui::link_muted(&db_path)),
    ]);

    ui::print_blank();
    ui::print_fieldset("General");
    ui::print_blank();
    ui::print_key_value_pairs(&[
        ("default_branch", ui::success(&cfg.general.default_branch)),
        (
            "auto_fetch",
            ui::paint_text(&cfg.general.auto_fetch.to_string()),
        ),
        (
            "pager",
            ui::muted(cfg.general.pager.as_deref().unwrap_or("(auto)")),
        ),
    ]);

    ui::print_blank();
    ui::print_fieldset("UI");
    ui::print_blank();
    ui::print_key_value_pairs(&[
        ("theme", ui::paint_text(&cfg.ui.theme)),
        ("colors", ui::paint_text(&cfg.ui.colors.to_string())),
        ("icons", ui::paint_text(&cfg.ui.icons.to_string())),
        ("date_format", ui::paint_text(&cfg.ui.date_format)),
        ("log_limit", ui::paint_text(&cfg.ui.log_limit.to_string())),
        ("show_graph", ui::paint_text(&cfg.ui.show_graph.to_string())),
        ("commit_mode", ui::paint_text(&cfg.ui.commit_mode)),
    ]);

    ui::print_blank();
    ui::print_fieldset("Commit");
    ui::print_blank();
    ui::print_key_value_pairs(&[
        (
            "require_scope",
            ui::paint_text(&cfg.commit.require_scope.to_string()),
        ),
        (
            "require_body",
            ui::paint_text(&cfg.commit.require_body.to_string()),
        ),
        (
            "max_subject",
            ui::paint_text(&cfg.commit.max_subject_length.to_string()),
        ),
        ("sign_off", ui::paint_text(&cfg.commit.sign_off.to_string())),
        ("gpg_sign", ui::paint_text(&cfg.commit.gpg_sign.to_string())),
        ("emoji", ui::paint_text(&cfg.commit.emoji.to_string())),
        ("types", ui::muted(&cfg.commit.types.join(", "))),
    ]);

    ui::print_blank();
    ui::print_fieldset("Diff");
    ui::print_blank();
    ui::print_key_value_pairs(&[
        ("tool", ui::paint_text(&cfg.diff.tool)),
        (
            "context_lines",
            ui::paint_text(&cfg.diff.context_lines.to_string()),
        ),
    ]);

    ui::print_blank();
    ui::print_fieldset("GitHub");
    ui::print_blank();
    ui::print_key_value_pairs(&[
        ("api_base", ui::paint_text(&cfg.github.api_base)),
        (
            "token",
            if cfg.github.token.is_some() {
                ui::success("*** (set)")
            } else {
                ui::muted("(not set)")
            },
        ),
    ]);

    ui::print_blank();
    ui::print_tip(&format!("{} config --edit  to open in $EDITOR", bin_name()));
    Ok(())
}

/// A theme available for selection: its name and whether it is a shipped
/// built-in (vs. a user-authored file under `~/.config/g/themes`).
struct ThemeChoice {
    name: String,
    builtin: bool,
}

/// Gather every selectable theme: the shipped built-ins first (in their
/// canonical order), then any custom `*.toml` files in the user themes
/// directory that do not shadow a built-in, sorted alphabetically.
fn gather_themes() -> Vec<ThemeChoice> {
    let builtins = ui::theme::builtin_names();
    let mut out: Vec<ThemeChoice> = builtins
        .iter()
        .map(|n| ThemeChoice {
            name: (*n).to_string(),
            builtin: true,
        })
        .collect();

    if let Some(dir) = ui::theme::themes_dir() {
        let mut customs: Vec<String> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("toml") {
                    p.file_stem().and_then(|s| s.to_str()).map(String::from)
                } else {
                    None
                }
            })
            .filter(|n| !builtins.contains(&n.as_str()))
            .collect();
        customs.sort();
        out.extend(customs.into_iter().map(|name| ThemeChoice {
            name,
            builtin: false,
        }));
    }
    out
}

/// Handle `g config --themes`.
///
/// In an interactive terminal this presents a picker of every recognised theme;
/// choosing one persists it to `[ui] theme` (preserving config comments) so the
/// choice is remembered.  In non-interactive contexts (piped output or
/// `--no-interactive`) it falls back to printing the list.
fn handle_themes() -> Result<()> {
    let cfg = config::load().unwrap_or_default();
    let themes = gather_themes();
    if themes.is_empty() {
        ui::print_warning("No themes found.");
        return Ok(());
    }

    let interactive = !ui::is_no_interactive() && std::io::stdin().is_terminal();

    if interactive {
        // Start the cursor on the currently-active theme.
        let current_idx = themes.iter().position(|t| t.name == cfg.ui.theme);
        let mut options: Vec<ui::SelectOption> = themes
            .iter()
            .map(|t| {
                let mut desc = if t.builtin { "built-in" } else { "custom" }.to_string();
                if t.name == cfg.ui.theme {
                    desc.push_str(" · current");
                }
                ui::SelectOption::with_description(&t.name, desc)
            })
            .collect();
        // Final "Create new theme..." entry — selecting it launches the
        // wizard instead of switching themes.
        let create_idx = options.len();
        options.push(ui::SelectOption::with_description(
            "+ Create new theme…",
            "wizard: pick a base, override colours, write a new TOML",
        ));

        let prompt = match current_idx {
            Some(_) => format!("Select a theme (current: {})", cfg.ui.theme),
            None => "Select a theme".to_string(),
        };

        if let Some(idx) = ui::select(&prompt, &options) {
            if idx == create_idx {
                return create_theme_wizard(None);
            }
            let chosen = &themes[idx].name;
            if *chosen == cfg.ui.theme {
                ui::print_blank();
                ui::print_info(&format!("Theme unchanged ({chosen})."));
            } else {
                config::set_theme(chosen)?;
                ui::print_blank();
                ui::print_success(&format!("Theme set to '{chosen}'."));
            }
            ui::print_blank();
            ui::print_tip("the new theme applies to your next command");
            return Ok(());
        }
        // Cancelled (Esc/q) — leave config untouched and say nothing noisy.
        return Ok(());
    }

    // Non-interactive: print the list with the active theme marked.
    ui::print_blank();
    ui::print_fieldset("Themes");
    ui::print_blank();
    for t in &themes {
        let marker = if t.name == cfg.ui.theme { ">" } else { " " };
        let kind = if t.builtin { "built-in" } else { "custom" };
        ui::print_line(&format!(
            "  {} {}  {}",
            ui::primary_bold(marker),
            t.name,
            ui::muted(&format!("({kind})"))
        ));
    }
    ui::print_blank();
    ui::print_tip(&format!(
        "{} config --themes  in a terminal to pick interactively",
        bin_name()
    ));
    Ok(())
}

// ─── g config get / set / list / menu ────────────────────────────────────────

/// Handle `g config --get <key>`.  Prints the exact value to stdout and exits
/// non-zero when the key is unknown or has no value set.
fn handle_config_get(key: &str) -> Result<()> {
    if config::settings::find(key).is_none() {
        ui::print_warning(&format!(
            "Unknown key '{key}' (see `{} config --list`).",
            bin_name()
        ));
        std::process::exit(1);
    }
    match config::settings::get(key)? {
        Some(v) => {
            ui::print_line(&v);
            Ok(())
        }
        None => {
            ui::print_warning(&format!("Key '{key}' is not set in the config file."));
            std::process::exit(1);
        }
    }
}

/// Handle `g config set <key> <value>` (validated + comment-preserving write).
fn handle_config_set(key: &str, value: &str) -> Result<()> {
    config::settings::set(key, value)?;
    ui::print_blank();
    ui::print_success(&format!(
        "{} = {}",
        ui::primary_bold(key),
        ui::warning(value)
    ));
    ui::print_blank();
    ui::print_tip(&format!("{} config --get {}  to confirm", bin_name(), key));
    Ok(())
}

/// Handle `g config --list` — render every editable scalar with its current
/// value, type and one-line help text.
fn handle_config_list() -> Result<()> {
    ui::print_blank();
    ui::print_fieldset("Editable settings");
    ui::print_blank();

    let max_key = config::settings::SCHEMA
        .iter()
        .map(|s| s.key.len())
        .max()
        .unwrap_or(0);

    for s in config::settings::SCHEMA {
        let current = config::settings::get(s.key)
            .ok()
            .flatten()
            .unwrap_or_else(|| "(unset)".to_string());
        // Pad the **uncoloured** key so that `:<width` doesn't count ANSI
        // escape bytes; then colour the already-padded string.
        let pad = " ".repeat(max_key.saturating_sub(s.key.len()));
        ui::print_line(&format!(
            "  {}{}  {}  {}",
            ui::primary(s.key),
            pad,
            ui::warning(&current),
            ui::muted(s.help),
        ));
    }
    ui::print_blank();
    ui::print_tip(&format!(
        "{} config set <key> <value>  to change one",
        bin_name()
    ));
    Ok(())
}

/// Handle `g config --menu` — interactive picker over the schema.
fn handle_config_menu() -> Result<()> {
    use std::io::IsTerminal;
    if ui::is_no_interactive() || !std::io::stdin().is_terminal() {
        // Non-interactive context — degrade to the listing view.
        return handle_config_list();
    }

    let entries: Vec<(&'static config::settings::Setting, String)> = config::settings::SCHEMA
        .iter()
        .map(|s| {
            let current = config::settings::get(s.key)
                .ok()
                .flatten()
                .unwrap_or_else(|| "(unset)".to_string());
            (s, current)
        })
        .collect();

    let options: Vec<ui::SelectOption> = entries
        .iter()
        .map(|(s, current)| {
            ui::SelectOption::with_description(s.key.to_string(), format!("= {current}"))
        })
        .collect();

    let Some(idx) = ui::select("Select a setting to change", &options) else {
        return Ok(());
    };
    let (setting, current) = &entries[idx];

    // Prompt for the new value using the right widget per kind.
    let new_value: Option<String> = match setting.kind {
        config::settings::Kind::Bool => {
            let yes = ui::confirm(
                &format!("{}  (current: {})", setting.key, current),
                current == "true",
            );
            Some(yes.to_string())
        }
        config::settings::Kind::Enum(choices) => {
            let opts: Vec<ui::SelectOption> = choices
                .iter()
                .map(|c| ui::SelectOption::new((*c).to_string()))
                .collect();
            ui::select(&format!("{}  (current: {})", setting.key, current), &opts)
                .map(|i| choices[i].to_string())
        }
        // Empty default — the prompt already shows the current value, so
        // pre-filling would force users to backspace before typing.
        config::settings::Kind::Int | config::settings::Kind::Str => {
            ui::input(&format!("{}  (current: {})", setting.key, current), None)
        }
    };

    let Some(v) = new_value else {
        ui::print_info("Cancelled.");
        return Ok(());
    };

    if v == *current {
        ui::print_info("Value unchanged.");
        return Ok(());
    }

    config::settings::set(setting.key, &v)?;
    ui::print_blank();
    ui::print_success(&format!(
        "{} = {}",
        ui::primary_bold(setting.key),
        ui::warning(&v)
    ));
    ui::print_blank();
    Ok(())
}

// ─── g config --new-theme: interactive theme wizard ─────────────────────────

/// The seven palette roles the wizard offers to override.  The eight
/// conventional-commit colors are intentionally omitted — they inherit from
/// the base theme and are rarely customised.
const PALETTE_ROLES: &[(&str, &str)] = &[
    ("primary", "info icon, spinner, active branch"),
    ("success", "checkmarks, added lines, current branch"),
    ("warning", "warnings, commit hashes, staged changes"),
    ("danger", "errors, deleted lines, remote refs"),
    ("muted", "dates, dividers, dim text"),
    ("text", "general body text"),
    ("accent", "section titles, tags, special refs"),
];

/// Interactive theme creator — drives the user through picking a base theme,
/// naming the new one, overriding selected palette roles, and choosing border
/// style + density, then writes the result to
/// `~/.config/g/themes/<name>.toml`.
///
/// When `name` is `Some`, that name is used directly; otherwise the user is
/// prompted for one.
fn create_theme_wizard(name: Option<&str>) -> Result<()> {
    use std::io::IsTerminal;
    if ui::is_no_interactive() || !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "Theme creation requires an interactive terminal. \
             Re-run without --no-interactive."
        );
    }

    ui::print_blank();
    ui::print_fieldset("Create new theme");
    ui::print_blank();

    // 1. Base theme — let the user extend any existing built-in or custom.
    let bases = gather_themes();
    let base_options: Vec<ui::SelectOption> = bases
        .iter()
        .map(|t| {
            ui::SelectOption::with_description(
                t.name.clone(),
                if t.builtin { "built-in" } else { "custom" },
            )
        })
        .collect();
    let Some(base_idx) = ui::select("Base theme to extend", &base_options) else {
        ui::print_info("Cancelled.");
        return Ok(());
    };
    let base = &bases[base_idx].name;

    // 2. Name.
    let dir = ui::theme::themes_dir()
        .ok_or_else(|| anyhow::anyhow!("could not determine themes directory"))?;
    let name = match name {
        Some(n) => n.to_string(),
        None => {
            let Some(n) = ui::input_validated("Name for the new theme", None, |raw| {
                let v = raw.trim();
                if v.is_empty() {
                    return Err("Name cannot be empty".into());
                }
                if v.contains('/') || v.ends_with(".toml") {
                    return Err("Name should be a plain identifier (no slashes, no .toml)".into());
                }
                Ok(())
            }) else {
                ui::print_info("Cancelled.");
                return Ok(());
            };
            n.trim().to_string()
        }
    };

    let target = dir.join(format!("{name}.toml"));
    if target.exists() {
        anyhow::bail!(
            "Theme file '{}' already exists. Pick a different name or delete it first.",
            target.display()
        );
    }

    // 3. Palette overrides — Enter to inherit, or type a colour to override.
    ui::print_blank();
    ui::print_info(
        "For each role: press Enter to inherit from the base, or type a colour \
         (hex like #88c0d0, an ANSI name like brightcyan, or a 256-colour index).",
    );
    let mut overrides: Vec<(&'static str, String)> = Vec::new();
    for (role, hint) in PALETTE_ROLES {
        let role_owned = role.to_string();
        let Some(input) = ui::input_validated(&format!("{role}  ({hint})"), None, move |raw| {
            let v = raw.trim();
            if v.is_empty() {
                return Ok(()); // inherit
            }
            ui::theme::parse_color(v)
                .map(|_| ())
                .map_err(|e| format!("{role_owned}: {e}"))
        }) else {
            ui::print_info("Cancelled.");
            return Ok(());
        };
        let trimmed = input.trim();
        if !trimmed.is_empty() {
            overrides.push((role, trimmed.to_string()));
        }
    }

    // 4. Border style — last option ("inherit") leaves the field absent.
    let border = pick_or_inherit(
        "Border style",
        &["sharp", "rounded", "heavy", "double", "ascii"],
    )?;

    // 5. Density.
    let density = pick_or_inherit("Density", &["normal", "compact", "relaxed"])?;

    // 6. Write the file.
    let mut body = String::new();
    body.push_str(&format!("# g — custom theme: {name}\n"));
    body.push_str(&format!(
        "# Created via `{} config --new-theme`.\n",
        bin_name()
    ));
    body.push_str(&format!("name = \"{name}\"\n"));
    body.push_str(&format!("extends = \"{base}\"\n"));
    if let Some(b) = &border {
        body.push_str(&format!("border_style = \"{b}\"\n"));
    }
    if let Some(d) = &density {
        body.push_str(&format!("density = \"{d}\"\n"));
    }
    if !overrides.is_empty() {
        body.push_str("\n[palette]\n");
        for (role, val) in &overrides {
            body.push_str(&format!("{role} = \"{val}\"\n"));
        }
    }

    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create themes directory '{}'", dir.display()))?;
    std::fs::write(&target, body)
        .with_context(|| format!("Failed to write '{}'", target.display()))?;

    ui::print_blank();
    ui::print_success(&format!(
        "Created theme '{}' at {}",
        ui::primary_bold(&name),
        ui::link_muted(&target.display().to_string())
    ));
    ui::print_blank();

    // 7. Offer to activate it.
    if ui::confirm(&format!("Activate '{}' now?", name), true) {
        config::set_theme(&name)?;
        ui::print_blank();
        ui::print_success(&format!("Theme set to '{}'.", name));
    } else {
        ui::print_tip(&format!(
            "{} config set ui.theme {}  to activate later",
            bin_name(),
            name
        ));
    }
    ui::print_blank();
    Ok(())
}

/// Helper for the wizard: select one of `choices` or pick "inherit from base"
/// (the trailing entry) which returns `Ok(None)`.
fn pick_or_inherit(prompt: &str, choices: &[&str]) -> Result<Option<String>> {
    let mut opts: Vec<ui::SelectOption> = choices
        .iter()
        .map(|c| ui::SelectOption::new((*c).to_string()))
        .collect();
    opts.push(ui::SelectOption::with_description(
        "inherit from base",
        "leave the field absent",
    ));
    let inherit_idx = opts.len() - 1;
    match ui::select(prompt, &opts) {
        Some(i) if i == inherit_idx => Ok(None),
        Some(i) => Ok(Some(choices[i].to_string())),
        None => Ok(None),
    }
}

// Telemetry names moved next to the enum definitions: see
// `impl Commands { fn telemetry_names() }` in `cli/mod.rs`, with each
// subcommand's `impl … { fn name() }` in its own `cli/<domain>.rs` file.
// Adding a new subcommand variant now only changes one file.
