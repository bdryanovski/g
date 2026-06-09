//! CLI argument definitions using `clap`.
//!
//! Tutorial overview:
//! - `clap` uses derive macros (`#[derive(Parser)]`, `#[derive(Subcommand)]`)
//!   to turn Rust structs/enums into a command-line interface.
//! - Struct fields become flags or positional args, based on `#[arg(...)]`.
//! - Enums represent subcommands, and nested enums represent sub-subcommands.
//!
//! Rust concepts used here:
//! - Derive macros that generate parsing code at compile time.
//! - Attributes (`#[command(...)]`, `#[arg(...)]`) to configure parsing.
//! - Enums with data (e.g., `Create { name, branch }`) to model subcommand payloads.

use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

// ─── Help styles ─────────────────────────────────────────────────────────────

/// Build the colour palette used for `--help` output.
///
/// Applied via `#[command(styles = get_styles())]` on the top-level [`Cli`]
/// struct.  Clap 4 calls this function when it first constructs the command
/// object, so the styles are applied consistently to every `--help` page in
/// the hierarchy.
///
/// | Element       | Style                    |
/// |---------------|--------------------------|
/// | Section heads | bold bright-white        |
/// | Usage line    | bold bright-white        |
/// | Literals      | bold bright-cyan         |
/// | Placeholders  | cyan                     |
/// | Errors        | bold bright-red          |
/// | Valid values  | bold bright-green        |
/// | Invalid       | bold bright-yellow       |
fn get_styles() -> clap::builder::Styles {
    use clap::builder::styling::{AnsiColor, Style, Styles};
    Styles::styled()
        .header(
            Style::new()
                .fg_color(Some(AnsiColor::BrightWhite.into()))
                .bold(),
        )
        .usage(
            Style::new()
                .fg_color(Some(AnsiColor::BrightWhite.into()))
                .bold(),
        )
        .literal(
            Style::new()
                .fg_color(Some(AnsiColor::BrightCyan.into()))
                .bold(),
        )
        .placeholder(Style::new().fg_color(Some(AnsiColor::Cyan.into())))
        .error(
            Style::new()
                .fg_color(Some(AnsiColor::BrightRed.into()))
                .bold(),
        )
        .valid(
            Style::new()
                .fg_color(Some(AnsiColor::BrightGreen.into()))
                .bold(),
        )
        .invalid(
            Style::new()
                .fg_color(Some(AnsiColor::BrightYellow.into()))
                .bold(),
        )
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
    styles = get_styles(),
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

// ─── Workspace ───────────────────────────────────────────────────────────────

/// Workspace-related subcommands for git worktree management.
#[derive(Subcommand)]
#[command(after_help = "Examples:\n\
                  \n\
                  \x20 g workspace list                        list all workspaces\n\
                  \x20 g workspace create feature              create workspace on a new branch\n\
                  \x20 g workspace create -b existing feature  use an existing branch\n\
                  \x20 g workspace switch                      fuzzy-pick a workspace\n\
                  \x20 g workspace switch api                  switch to a named workspace\n\
                  \x20 g workspace status                      show current workspace info\n\
                  \x20 g workspace rename old new              rename a workspace\n\
                  \x20 g workspace delete api                  remove a workspace")]
pub enum WorkspaceCommands {
    /// Reorganise an existing repo into a container/worktree layout
    ///
    /// Moves the repo root into a new sub-directory named after the default
    /// branch (e.g. `main`), then recreates the original path as a container
    /// directory.  After `init`, new workspaces created with `g workspace
    /// create` are placed inside the container.
    Init,

    /// List all workspaces (git worktrees)
    List,

    /// Create a new workspace as a sibling worktree directory
    Create {
        /// Name for the new workspace
        name: String,
        /// Branch to check out (defaults to creating a new branch with the workspace name)
        #[arg(short = 'b', long)]
        branch: Option<String>,
        /// Starting commit or tag when creating a new branch (e.g. `abc1234`)
        start_point: Option<String>,
        /// Description of this workspace
        #[arg(short, long)]
        description: Option<String>,
        /// Show an interactive picker to copy untracked/gitignored files into the new workspace
        #[arg(long)]
        copy: bool,
    },

    /// Open a subshell in a workspace directory
    Switch {
        /// Workspace name (fuzzy matched). Omit to open an interactive picker.
        name: Option<String>,
    },

    /// Remove a workspace (git worktree remove)
    Delete {
        /// Workspace name
        name: String,
        /// Force removal even if the worktree is dirty
        #[arg(long)]
        force: bool,
    },

    /// Show current workspace info
    Status,

    /// Rename a workspace (move directory and repair worktree)
    Rename {
        /// Current name
        old: String,
        /// New name
        new: String,
    },
}

// ─── Stack ────────────────────────────────────────────────────────────────────

/// Stack-related subcommands for stacked PR workflows.
#[derive(Subcommand)]
#[command(after_help = "Workflow overview:\n\
                  \n\
                  \x20 1. g stack new my-feature    create a stack from current branch\n\
                  \x20 2. g stack add next-step      add a dependent branch on top\n\
                  \x20 3. (work, commit on each branch)\n\
                  \x20 4. g stack sync               rebase all branches in order\n\
                  \x20 5. g stack push               push all branches\n\
                  \x20 6. g stack pr                 open GitHub PRs for every branch\n\
                  \n\
                  Other useful commands:\n\
                  \n\
                  \x20 g stack view                  show the stack as a tree\n\
                  \x20 g stack details               show per-branch commit lists\n\
                  \x20 g stack squash                squash current branch to one commit\n\
                  \x20 g stack fold                  merge current branch into its parent")]
pub enum StackCommands {
    /// Initialize a new stack starting from the current branch
    New {
        /// Name for this stack
        name: String,
    },

    /// Create a new branch on top of the current stack
    Add {
        /// Branch name to create
        branch: String,
    },

    /// List all stacks
    List,

    /// Show the current stack as a tree
    View,

    /// Show the current stack with commits for each branch
    Details,

    /// Switch to a different stack (checks out its top branch)
    Switch {
        /// Stack name to switch to
        name: String,
    },

    /// Merge the current branch into the one below it in the stack
    Absorb,

    /// Squash the current branch to one commit on top of its base, then rebase branches above
    Squash {
        /// Commit message for the squashed commit (default: oldest commit subject in the range)
        #[arg(short, long)]
        message: Option<String>,
        /// Abort if any conflict is found instead of pausing
        #[arg(long)]
        no_interactive: bool,
    },

    /// Merge the current branch into its parent (preserving history), drop the extra ref, restack above
    Fold {
        /// Keep the current branch name as the combined branch (remove the parent ref from the stack)
        #[arg(long)]
        keep: bool,
        /// Abort if merge/rebase hits conflicts instead of pausing for resolution
        #[arg(long)]
        no_interactive: bool,
    },

    /// Sync all stack branches (rebase each on the one below)
    Sync {
        /// Abort if any conflict is found instead of pausing
        #[arg(long)]
        no_interactive: bool,
    },

    /// Push all branches in the current stack
    Push {
        /// Force push with lease
        #[arg(long)]
        force: bool,
    },

    /// Create or update GitHub PRs for all branches in the stack
    Pr {
        /// Open PRs in browser after creating
        #[arg(long)]
        open: bool,
        /// Draft PRs
        #[arg(long)]
        draft: bool,
    },

    /// Remove a branch from the stack (doesn't delete the branch)
    Remove {
        /// Branch name to remove from stack
        branch: String,
    },

    /// Delete a stack (and optionally its branches)
    Delete {
        /// Stack name
        name: String,
        /// Also delete all branches in the stack
        #[arg(long)]
        branches: bool,
    },

    /// Move a stack up or down in the stack list (affects display order and PR ordering)
    Up,
    Down,
}

// ─── Commit ───────────────────────────────────────────────────────────────────

/// Arguments for the interactive and non-interactive commit flow.
#[derive(Args)]
pub struct CommitArgs {
    /// Commit message subject (skips interactive mode)
    #[arg(short, long)]
    pub message: Option<String>,

    /// Commit message body
    #[arg(short, long)]
    pub body: Option<String>,

    /// Commit type (feat, fix, docs, etc.) — skips prompt
    #[arg(long)]
    // `type` is a Rust keyword, so we use a raw identifier: `r#type`.
    pub r#type: Option<String>,

    /// Commit scope — skips prompt
    #[arg(long)]
    pub scope: Option<String>,

    /// Don't run pre-commit hooks
    #[arg(long)]
    pub no_verify: bool,

    /// Stage all changes before committing
    #[arg(short = 'a', long)]
    pub all: bool,

    /// Amend the last commit
    #[arg(long)]
    pub amend: bool,
}

// ─── Compare ─────────────────────────────────────────────────────────────────

/// Arguments for comparing two branches.
#[derive(Args)]
pub struct CompareArgs {
    /// Base branch (defaults to main/master)
    pub base: Option<String>,

    /// Head branch (defaults to current)
    pub head: Option<String>,

    /// Only show file-level stat, not full diff
    #[arg(long)]
    pub stat: bool,

    /// Show full diff
    #[arg(long)]
    pub diff: bool,

    /// Show only commits
    #[arg(long)]
    pub commits: bool,
}

// ─── Git pass-through ─────────────────────────────────────────────────────────

/// Pass-through args used by enhanced git commands.
#[derive(Args)]
pub struct GitPassArgs {
    /// Extra arguments forwarded to git
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

// ─── Branch ───────────────────────────────────────────────────────────────────

/// `g branch` with optional `squash` subcommand; other tokens go to list / `git branch` passthrough.
#[derive(Args)]
pub struct BranchArgs {
    #[command(subcommand)]
    pub cmd: Option<BranchSquashCmd>,
    /// When no `squash` subcommand: forwarded to enhanced list or `git branch`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub rest: Vec<String>,
}

#[derive(Subcommand)]
pub enum BranchSquashCmd {
    /// Collapse all commits on the current branch into one (from merge-base with base)
    Squash {
        /// Commit message (default: oldest subject in the squashed range)
        #[arg(short, long)]
        message: Option<String>,
        /// Ref to merge against when finding the fork point (`git merge-base HEAD <base>`)
        #[arg(short = 'b', long)]
        base: Option<String>,
    },
}

// ─── Config ───────────────────────────────────────────────────────────────────

/// Arguments for configuration-related commands.
#[derive(Args)]
pub struct ConfigArgs {
    /// Open config file in $EDITOR
    #[arg(long)]
    pub edit: bool,

    /// Print the path to the config file
    #[arg(long)]
    pub path: bool,

    /// Get a config value
    pub key: Option<String>,
}

// ─── Stats ────────────────────────────────────────────────────────────────────

/// Arguments for the `g stats` command.
#[derive(Args)]
#[command(after_help = "Examples:\n\
                  \n\
                  \x20 g stats                    full report, last 365 days\n\
                  \x20 g stats --days 90           last 90 days\n\
                  \x20 g stats --no-git            skip sections that require git\n\
                  \x20 g stats --import            import git history for commit analysis\n\
                  \x20 g stats --search \"fix bug\"  fuzzy search commit messages\n\
                  \x20 g stats --duplicates        show duplicate commit messages\n")]
pub struct StatsArgs {
    /// Number of days to look back for time-based stats
    #[arg(long, default_value = "365", value_name = "DAYS")]
    pub days: u32,

    /// Skip sections that require a git repository (heatmap, lines chart)
    #[arg(long)]
    pub no_git: bool,

    /// Import git commit history into the statistics database
    #[arg(long)]
    pub import: bool,

    /// Maximum number of commits to import (default: all)
    #[arg(long, value_name = "N")]
    pub import_limit: Option<usize>,

    /// Search commit messages using fuzzy matching
    #[arg(long, value_name = "QUERY")]
    pub search: Option<String>,

    /// Show duplicate commit messages
    #[arg(long)]
    pub duplicates: bool,

    /// Show commit message length statistics and trends
    #[arg(long)]
    pub message_stats: bool,
}

// ─── Developer ───────────────────────────────────────────────────────────────

/// Developer / debugging utilities for inspecting internal tool state.
#[derive(Subcommand)]
pub enum DeveloperCommands {
    /// Open an interactive SQLite shell connected to the internal g.db database
    ///
    /// Launches `sqlite3` with the path to `~/.config/g/g.db` so you can run
    /// arbitrary SQL queries for debugging.  Pass `--path` to print the
    /// database path without opening a shell.
    Db {
        /// Print the database path and exit (don't open the shell)
        #[arg(long)]
        path: bool,
    },

    /// List all repositories tracked in the internal database
    ///
    /// Shows every repo root path that has been seen by the tool, along with
    /// the first and most recent time it was active.
    Repos,
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
