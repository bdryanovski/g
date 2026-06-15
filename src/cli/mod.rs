//! CLI argument definitions using `clap`.
//!
//! Tutorial overview:
//! - `clap` uses derive macros (`#[derive(Parser)]`, `#[derive(Subcommand)]`)
//!   to turn Rust structs/enums into a command-line interface.
//! - Struct fields become flags or positional args, based on `#[arg(...)]`.
//! - Enums represent subcommands, and nested enums represent sub-subcommands.
//!
//! # Folder layout
//!
//! ```text
//! cli/
//!   mod.rs        ← this file: Cli + Commands + GitPassArgs +
//!                   print_completions + Commands::telemetry_names + re-exports
//!   styles.rs     ← help-output colour palette
//!   workspace.rs  ← WorkspaceCommands
//!   stack.rs      ← StackCommands
//!   commit.rs     ← CommitArgs
//!   compare.rs    ← CompareArgs
//!   branch.rs     ← BranchArgs + BranchSquashCmd
//!   config.rs     ← ConfigArgs
//!   stats.rs      ← StatsArgs
//!   developer.rs  ← DeveloperCommands
//! ```
//!
//! Every per-domain struct/enum is re-exported below so external call sites
//! continue to write `use crate::cli::CommitArgs;` (etc.) without caring
//! about the internal split.

use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

mod branch;
mod commit;
mod compare;
mod config;
mod developer;
mod stack;
mod stats;
mod styles;
mod workspace;

// ── Public re-exports — preserve every `crate::cli::X` path used elsewhere.
pub use branch::BranchArgs;
// `BranchSquashCmd` is destructured at runtime by `commands::git::dispatch_branch`
// but the build script only walks the clap tree via `Cli::command()`, so the
// re-export looks unused there — allow it for that compilation unit.
#[allow(unused_imports)]
pub use branch::BranchSquashCmd;
pub use commit::CommitArgs;
pub use compare::CompareArgs;
pub use config::ConfigArgs;
// `ConfigCmd` is destructured at runtime by `main::handle_config` but the
// build script only walks the clap tree via `Cli::command()`, so the
// re-export looks unused there — allow it for that compilation unit.
#[allow(unused_imports)]
pub use config::ConfigCmd;
pub use developer::DeveloperCommands;
pub use stack::StackCommands;
pub use stats::StatsArgs;
pub use workspace::WorkspaceCommands;

// ─── Git pass-through ─────────────────────────────────────────────────────────

/// Pass-through args used by every enhanced git command (`g log`, `g status`,
/// `g diff`, `g show`, `g add`).  Lives in `mod.rs` because the `Commands`
/// enum below references it from six variants — keeping it local avoids a
/// trivial one-struct sub-file.
#[derive(Args)]
pub struct GitPassArgs {
    /// Extra arguments forwarded to git
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

// ─── Top-level CLI ────────────────────────────────────────────────────────────

/// A beautiful Git CLI with stacked PRs, workspace management, and enhanced UX.
/// All standard git commands are passed through transparently.
///
/// The `name` attribute is intentionally absent so that clap reads the binary
/// name from the first element of `try_parse_from` at runtime (set in
/// `main::run`).  This means `--help` and error messages always show the
/// actual name of the executable — rename the binary and everything updates
/// automatically.
#[derive(Parser)]
#[command(
    about = "Enhanced Git with stacked PRs, workspaces, and beautiful output",
    long_about = "An enhanced Git CLI that layers powerful workflows on top of git.\n\
                  \n\
                  All standard git commands are forwarded transparently — just swap\n\
                  'git' for this tool and everything continues to work as expected.\n\
                  \n\
                  Enhanced commands add colour, icons, and smarter output.  New\n\
                  commands add stacked-PR workflows, parallel worktree workspaces,\n\
                  and an interactive conventional-commit builder.",
    after_help = "Examples:\n\
                  \n\
                  \x20 g log                       enhanced git log with graph\n\
                  \x20 g status                    enhanced status with icons\n\
                  \x20 g add                       interactive file picker to stage\n\
                  \x20 g commit                    interactive conventional commit\n\
                  \x20 g diff                      enhanced diff\n\
                  \x20 g branch                    list branches with ahead/behind\n\
                  \x20 g compare main              compare current branch to main\n\
                  \x20 g stack new my-feature      start a stacked PR workflow\n\
                  \x20 g workspace create api      create a parallel workspace\n\
                  \n\
                  Pass --help to any subcommand for detailed usage and examples.",
    version,
    propagate_version = true,
    color = clap::ColorChoice::Auto,
    styles = styles::get_styles(),
)]
pub struct Cli {
    /// Run as if git was started in <path>
    #[arg(short = 'C', global = true, value_name = "PATH")]
    pub directory: Option<String>,

    /// Override a configuration value (key=value)
    #[arg(short = 'c', global = true, value_name = "KEY=VAL")]
    pub config_override: Vec<String>,

    /// Preview what commands would run without making any changes
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Disable all interactive TUI prompts; use defaults or require --flag values.
    /// Useful for scripting and CI environments.
    #[arg(long, global = true)]
    pub no_interactive: bool,

    /// The parsed top-level command chosen by the user.
    #[command(subcommand)]
    pub command: Commands,
}

// ─── Top-level commands ───────────────────────────────────────────────────────

/// Top-level command set for `g`.
#[derive(Subcommand)]
pub enum Commands {
    /// Manage worktree-based workspaces (parallel branch checkouts)
    #[command(subcommand)]
    Workspace(WorkspaceCommands),

    /// Manage stacked pull requests
    #[command(subcommand)]
    Stack(StackCommands),

    /// Interactive guided commit with message templates
    Commit(CommitArgs),

    /// Stage files interactively, or forward arguments to `git add`
    ///
    /// With no arguments an interactive multi-select picker is shown so you
    /// can choose exactly which files to stage using ↑↓ / j k and Space.
    /// Any flags or paths you supply are forwarded to `git add` unchanged.
    Add(GitPassArgs),

    /// Interactive file-tree picker for staging and unstaging
    ///
    /// Opens a full-screen tree view of every changed file (staged, unstaged
    /// and untracked).  Navigate with j/k, toggle with Space, confirm with
    /// Enter.  Press `d` on any tracked file to revert it to its last known
    /// state.
    ///
    /// Already-staged files start pre-checked so running `g stage` a second
    /// time lets you adjust your selection without losing what you staged.
    Stage,

    /// Compare two branches visually
    Compare(CompareArgs),

    /// Enhanced git log with beautiful formatting
    Log(GitPassArgs),

    /// Enhanced git status with icons and colors
    Status(GitPassArgs),

    /// Enhanced git diff using your configured diff tool
    Diff(GitPassArgs),

    /// Enhanced branch listing, `git branch` passthrough, or `branch squash`
    Branch(BranchArgs),

    /// Enhanced git show
    Show(GitPassArgs),

    /// Open interactive config editor
    Config(ConfigArgs),

    /// Display a rich usage-statistics report
    ///
    /// Aggregates data from the internal SQLite database (command runs, commits
    /// recorded via `g commit`) and the current repository's git history to
    /// produce a terminal report that includes:
    ///
    ///   • Overview totals and streak information
    ///   • GitHub-style commit heatmap for the last 52 weeks
    ///   • Lines-added / lines-removed sparkline (current branch)
    ///   • Top commands by frequency
    ///   • Conventional-commit type distribution
    ///   • Repository activity ranking
    ///   • Activity-by-hour chart
    Stats(StatsArgs),

    /// Developer / debugging utilities
    #[command(subcommand)]
    Developer(DeveloperCommands),

    /// Print a shell completion script and exit
    ///
    /// Pipe the output to the right location for your shell:
    ///
    ///   Bash:  g completions bash  >> ~/.bash_completion
    ///   Zsh:   g completions zsh   >  ~/.zsh/completions/_g
    ///   Fish:  g completions fish  >  ~/.config/fish/completions/g.fish
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Passthrough: all other git commands are forwarded transparently
    #[command(external_subcommand)]
    // This collects any unknown subcommand + args into a Vec<String>.
    Git(Vec<String>),
}

// TODO(cli): Add `--json` output flags for commands that are easy to serialize (list/status).

/// Write a shell completion script for `shell` to stdout.
///
/// Called from `main.rs` when the user runs `g completions <shell>`.
pub fn print_completions(shell: Shell) {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    clap_complete::generate(shell, &mut cmd, name, &mut std::io::stdout());
}

// ─── Telemetry names ──────────────────────────────────────────────────────────

impl Commands {
    /// Return `(top_level, subcommand)` static names for stats recording.
    ///
    /// Co-located with the enum definitions so adding a new variant or
    /// subcommand requires updating exactly one place (each domain owns its
    /// own `impl … { fn name() }` in its sub-file).
    ///
    /// Examples:
    /// - `g commit`            → `("commit", None)`
    /// - `g workspace create`  → `("workspace", Some("create"))`
    /// - `g stack pr`          → `("stack", Some("pr"))`
    pub fn telemetry_names(&self) -> (&'static str, Option<&'static str>) {
        match self {
            Self::Workspace(sub) => ("workspace", Some(sub.name())),
            Self::Stack(sub) => ("stack", Some(sub.name())),
            Self::Developer(sub) => ("developer", Some(sub.name())),
            Self::Commit(_) => ("commit", None),
            Self::Add(_) => ("add", None),
            Self::Stage => ("stage", None),
            Self::Compare(_) => ("compare", None),
            Self::Log(_) => ("log", None),
            Self::Status(_) => ("status", None),
            Self::Diff(_) => ("diff", None),
            Self::Branch(_) => ("branch", None),
            Self::Show(_) => ("show", None),
            Self::Stats(_) => ("stats", None),
            Self::Config(_) => ("config", None),
            Self::Completions { .. } => ("completions", None),
            // Dynamic passthrough — subcommand isn't a known &'static str.
            Self::Git(_) => ("git", None),
        }
    }
}
