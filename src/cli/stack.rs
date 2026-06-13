//! `g stack …` subcommand definitions.

use clap::Subcommand;

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

impl StackCommands {
    /// Static name used for telemetry / stats recording.
    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::New { .. } => "new",
            Self::Add { .. } => "add",
            Self::List => "list",
            Self::View => "view",
            Self::Details => "details",
            Self::Switch { .. } => "switch",
            Self::Absorb => "absorb",
            Self::Squash { .. } => "squash",
            Self::Fold { .. } => "fold",
            Self::Sync { .. } => "sync",
            Self::Push { .. } => "push",
            Self::Pr { .. } => "pr",
            Self::Remove { .. } => "remove",
            Self::Delete { .. } => "delete",
            Self::Up => "up",
            Self::Down => "down",
        }
    }
}
