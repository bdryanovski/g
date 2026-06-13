//! `g workspace …` subcommand definitions.

use clap::Subcommand;

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

impl WorkspaceCommands {
    /// Static name used for telemetry / stats recording.
    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::List => "list",
            Self::Create { .. } => "create",
            Self::Switch { .. } => "switch",
            Self::Delete { .. } => "delete",
            Self::Status => "status",
            Self::Rename { .. } => "rename",
        }
    }
}
