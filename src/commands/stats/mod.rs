//! Usage-statistics report — `g stats`.
//!
//! Aggregates data from two sources:
//!
//! 1. **SQLite database** (`~/.config/g/g.db`) — command runs, commits recorded
//!    via `g commit`, branch events, workspace events, stack events.
//! 2. **`git log`** — real commit dates (all branches) for the heatmap, and
//!    per-commit line-change counts for the sparkline chart.
//!
//! # Folder layout
//!
//! ```text
//! stats/
//!   mod.rs           ← this file: public `stats()` entry point + routing
//!   shared.rs        ← fmt helpers + the generic horizontal bar chart
//!   overview.rs      ← Usage Overview section
//!   heatmap.rs       ← 52-week commit heatmap
//!   lines_chart.rs   ← +/- lines sparkline (last 60 commits)
//!   top_commands.rs  ← Top Commands bar chart
//!   commit_types.rs  ← Conventional-commit type distribution
//!   repo_activity.rs ← Repository activity ranking
//!   active_hours.rs  ← Activity-by-hour vertical chart
//!   messages.rs      ← `--message-stats` mode: length stats, trends, top authors
//!   import.rs        ← `--import` mode: backfill git history into the db
//!   search.rs        ← `--search` mode: fuzzy search of imported commits
//!   duplicates.rs    ← `--duplicates` mode: repeated commit subjects
//! ```
//!
//! Each section is its own focused file. Cross-section helpers (number
//! formatting, the generic bar chart) live in [`shared`] and are
//! `pub(super)`-scoped — they never leak outside `stats/`.

use anyhow::Result;

use crate::cli::StatsArgs;
use crate::commands::Ctx;
use crate::ui;

mod active_hours;
mod commit_types;
mod duplicates;
mod heatmap;
mod import;
mod lines_chart;
mod messages;
mod overview;
mod repo_activity;
mod search;
mod shared;
mod top_commands;

/// Render the full usage-statistics report (or one of the special modes when
/// the corresponding flag is set).
///
/// # Errors
///
/// Returns an error if the database cannot be queried.
pub fn stats(ctx: &Ctx, args: &StatsArgs) -> Result<()> {
    // Special modes — each consumes its own flag and returns directly.
    if args.import {
        return import::run(ctx, args.import_limit);
    }
    if let Some(ref query) = args.search {
        return search::run(ctx, query);
    }
    if args.duplicates {
        return duplicates::run(ctx);
    }

    // Full report.
    ui::print_blank();

    overview::run(ctx)?;

    if !args.no_git {
        heatmap::run();
        lines_chart::run();
    }

    top_commands::run(ctx)?;
    commit_types::run(ctx)?;
    repo_activity::run(ctx)?;
    active_hours::run(ctx)?;

    if args.message_stats {
        messages::length_stats(ctx)?;
        messages::length_trends(ctx)?;
        messages::top_authors(ctx)?;
        messages::top_duplicates(ctx)?;
    }

    ui::print_blank();
    Ok(())
}
