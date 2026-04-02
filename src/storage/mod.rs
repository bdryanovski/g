//! Persistent storage layer backed by SQLite.
//!
//! ## Architecture
//!
//! A single [`rusqlite::Connection`] is opened once per CLI invocation in
//! `main::run()` and passed by reference to every command that needs
//! persistence.  There is no connection pool — this is a single-threaded CLI.
//!
//! ## Modules
//!
//! - [`db`] — open the connection, apply PRAGMAs, run migrations.
//! - [`migrations`] — versioned SQL migration runner.
//! - [`repos`] — upsert/lookup repo anchor rows (shared FK for all tables).
//! - [`workspaces`] — CRUD for git worktree metadata (replaces `workspaces.toml`).
//! - [`stacks`] — CRUD for stacked-PR metadata (replaces `stacks.toml`).
//! - [`stats`] — append-only event recording for command usage and activity.
//! - [`toml_import`] — one-time import of legacy TOML data on first run.

pub mod db;
pub mod repos;
pub mod stacks;
pub mod stats;
pub mod workspaces;

mod migrations;
mod toml_import;

// Re-export the most commonly used types so callers can write
// `use crate::storage::{WorkspaceRow, StackRow, …}` without drilling into
// sub-modules.
pub use stacks::{StackBranchRow, StackRow};
pub use workspaces::WorkspaceRow;
