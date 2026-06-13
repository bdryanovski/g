//! `g branch …` arguments — the enhanced branch list, `git branch`
//! passthrough, and the `branch squash` subcommand.

use clap::{Args, Subcommand};

/// `g branch` with optional `squash` subcommand; other tokens go to list /
/// `git branch` passthrough.
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
