//! Command implementations grouped by feature area.
//!
//! Tutorial overview:
//! - `mod` declares a module; `pub mod` re-exports it so other modules can use it.
//! - Each submodule is defined in its own file (e.g., `commands/commit.rs`).
//! - This file acts like a "directory index" for the `commands` namespace.

pub mod commit;
pub mod compare;
pub mod git;
pub mod stack;
pub mod workspace;

// TODO(commands): Consider a shared error type for consistent UX and easier testing.
