//! `g commit` arguments.

use clap::Args;

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
