//! `g notes` — manage private review notes left from the diff TUI.
//!
//! These notes are the "private" bucket of the two-bucket review-comment
//! model: they never leave the local SQLite DB.  Use `g diff` (with the `c`
//! key inside the TUI) to create them; this command surfaces ways to list,
//! inspect, edit, and clear them from outside the TUI.

use clap::Subcommand;

/// Subcommand surface for `g notes` — also the variant payload in
/// `Commands::Notes`, mirroring how `WorkspaceCommands` and `StackCommands`
/// are wired.
#[derive(Debug, Subcommand)]
pub enum NotesCommands {
    /// List every saved note in this repo (newest first).
    List,
    /// Show a single note by id — prints the body and its anchor.
    Show { id: i64 },
    /// Edit the body of an existing note in `$EDITOR`.
    Edit { id: i64 },
    /// Delete a single note by id.
    Delete { id: i64 },
    /// Delete all notes for a given file (or every note when no path is given).
    /// Requires `--force` to actually run — protects against fat-finger typos.
    Clear {
        /// Restrict to this repo-relative file path (optional).
        path: Option<String>,
        /// Skip the confirmation prompt.
        #[arg(long)]
        force: bool,
    },
    /// Publish a saved private note to the GitHub PR for the current branch.
    ///
    /// The "public bucket" of the two-bucket model: this posts the note body
    /// as a line-anchored review comment on the open PR whose head matches the
    /// current branch.  Requires `GITHUB_TOKEN` in the environment (or a token
    /// set in `[github].token`) and an open PR on the current branch.
    Publish { id: i64 },
}

impl NotesCommands {
    /// Return the subcommand name for telemetry (`Self::Notes(sub) => sub.name()`).
    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Show { .. } => "show",
            Self::Edit { .. } => "edit",
            Self::Delete { .. } => "delete",
            Self::Clear { .. } => "clear",
            Self::Publish { .. } => "publish",
        }
    }
}