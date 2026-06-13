//! Append-only event recording for usage statistics, plus read-side queries.
//!
//! # Folder layout
//!
//! ```text
//! stats/
//!   mod.rs       ← this file: module wiring + public re-exports
//!   events.rs    ← write-side: record_command, record_branch_event,
//!                  record_commit, record_workspace_event, record_stack_event,
//!                  record_commit_message, import_git_history
//!   queries.rs   ← read-side aggregations: OverallStats, query_overall,
//!                  top_commands, commit_type_counts, top_repos_by_activity,
//!                  activity_by_hour, streak_info
//!   messages.rs  ← commit-message queries: search, duplicates, length stats,
//!                  monthly trends, top authors
//! ```
//!
//! ## Conventions
//!
//! - All `record_*` functions are designed to be called with `.ok()` — a stats
//!   write failure must never abort the user's actual operation.
//! - All `query_*` / `top_*` / `streak_info` / `activity_by_hour` helpers are
//!   read-only and tolerant of missing or empty tables: a failing query
//!   returns the zero value rather than an error so the rest of the report
//!   keeps rendering.
//!
//! Every public symbol below is re-exported under `crate::storage::stats::*`
//! so call sites continue to write `use crate::storage::stats as db;` and
//! `db::top_commands(…)` without caring about the split.

mod events;
mod messages;
mod queries;

// Write-side: append-only event recording.
pub use events::{
    import_git_history, record_branch_event, record_command, record_commit, record_commit_message,
    record_stack_event, record_workspace_event,
};

// Read-side: overview + ranked aggregations.
pub use queries::{
    activity_by_hour, commit_type_counts, query_overall, streak_info, top_commands,
    top_repos_by_activity,
};

// Types and items reached only via return-type inference or kept as future API
// surface — explicitly allowed to silence the dead-code lint at the re-export.
#[allow(unused_imports)]
pub use queries::{command_run_counts_per_day, OverallStats};

// Read-side: commit-message queries.
pub use messages::{
    commit_length_over_time, commit_length_stats, find_duplicate_commits, search_commits,
    top_authors, total_commit_messages,
};
#[allow(unused_imports)]
pub use messages::{CommitLengthStats, CommitSearchResult, MonthlyLengthStats};
