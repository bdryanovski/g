//! Command implementations grouped by feature area.
//!
//! Tutorial overview:
//! - This file acts like a "directory index" for the `commands` namespace.
//! - It defines and exports submodules for each major feature area:
//!   `commit`, `compare`, `git`, `stack`, and `workspace`.
//!
//! Rust concepts used here:
//! - `mod` declares a module; `pub mod` re-exports it so other modules can use it.
//! - Each submodule is defined in its own file (e.g., `commands/commit.rs`).

pub mod commit;
pub mod compare;
pub mod developer;
pub mod git;
pub mod stack;
pub mod stage;
pub mod workspace;

// TODO(commands): Consider a shared error type for consistent UX and easier testing.
