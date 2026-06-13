//! `g compare` arguments.

use clap::Args;

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
