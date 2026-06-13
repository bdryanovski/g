//! `g developer …` subcommands — internal debug utilities.

use clap::Subcommand;

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

impl DeveloperCommands {
    /// Static name used for telemetry / stats recording.
    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::Db { .. } => "db",
            Self::Repos => "repos",
        }
    }
}
