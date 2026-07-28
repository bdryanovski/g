//! CLI argument definitions for `g hooks` subcommands.

use clap::{Args, Subcommand};

/// Commands for managing personal git hooks.
#[derive(Subcommand)]
pub enum HooksCommands {
    /// List all configured hooks for this repository
    List,

    /// Run a specific hook manually
    Run(HooksRunArgs),

    /// Create a hooks.toml template in .g/
    Init,

    /// Show where hooks config is loaded from
    Status,
}

impl HooksCommands {
    /// Return the subcommand name for telemetry.
    pub fn name(&self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Run(_) => "run",
            Self::Init => "init",
            Self::Status => "status",
        }
    }
}

/// Arguments for `g hooks run`.
#[derive(Args)]
pub struct HooksRunArgs {
    /// Hook name to run (pre-commit, post-commit, pre-push, etc.)
    pub hook: String,

    /// Skip the hook if no files match patterns
    #[arg(long)]
    pub skip_empty: bool,
}
