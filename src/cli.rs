use clap::{Parser, Subcommand, Args};

/// vcli — A beautiful Git CLI with stacked PRs, workspace management, and enhanced UX.
/// All standard git commands are passed through transparently.
#[derive(Parser)]
#[command(
    name = "vcli",
    about = "Version CLI — enhanced Git with stacked PRs, workspaces, and beautiful output",
    long_about = None,
    version,
    propagate_version = true,
    color = clap::ColorChoice::Auto,
)]
pub struct Cli {
    /// Run as if git was started in <path>
    #[arg(short = 'C', global = true, value_name = "PATH")]
    pub directory: Option<String>,

    /// Override a configuration value (key=value)
    #[arg(short = 'c', global = true, value_name = "KEY=VAL")]
    pub config_override: Vec<String>,

    #[command(subcommand)]
    pub command: Commands,
}

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

    /// Compare two branches visually
    Compare(CompareArgs),

    /// Enhanced git log with beautiful formatting
    Log(GitPassArgs),

    /// Enhanced git status with icons and colors
    Status(GitPassArgs),

    /// Enhanced git diff using your configured diff tool
    Diff(GitPassArgs),

    /// Enhanced branch listing with metadata
    Branch(GitPassArgs),

    /// Enhanced git show
    Show(GitPassArgs),

    /// Open interactive config editor
    Config(ConfigArgs),

    /// Passthrough: all other git commands are forwarded transparently
    #[command(external_subcommand)]
    Git(Vec<String>),
}

// ─── Workspace ───────────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum WorkspaceCommands {
    /// List all workspaces (git worktrees)
    List,

    /// Create a new workspace as a sibling worktree directory
    Create {
        /// Name for the new workspace
        name: String,
        /// Branch to check out (defaults to creating a new branch with the workspace name)
        #[arg(short, long)]
        branch: Option<String>,
        /// Description of this workspace
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Open a subshell in a workspace directory
    Switch {
        /// Workspace name (fuzzy matched)
        name: String,
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

#[derive(Subcommand)]
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
}

// ─── Commit ───────────────────────────────────────────────────────────────────

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

#[derive(Args)]
pub struct GitPassArgs {
    /// Extra arguments forwarded to git
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

// ─── Config ───────────────────────────────────────────────────────────────────

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
