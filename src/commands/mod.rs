//! Command implementations grouped by feature area.
//!
//! Each submodule implements the logic behind a top-level CLI command.

pub mod commit;
pub mod compare;
pub mod git;
pub mod stack;
pub mod workspace;

// TODO(commands): Consider a shared error type for consistent UX and easier testing.
