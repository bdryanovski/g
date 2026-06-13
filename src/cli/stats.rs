//! `g stats` arguments.

use clap::Args;

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
