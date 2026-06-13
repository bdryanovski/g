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
pub mod ctx;
pub mod developer;
pub mod error;
pub mod git;
pub mod prelude;
pub mod stack;
pub mod stage;
pub mod stats;
pub mod workspace;

// Test-only shared fixtures.  Compiled away in non-test builds.
#[cfg(test)]
mod test_support;

/// The per-invocation runtime context handed to every command.
///
/// Re-exported here so call sites can write `commands::Ctx` instead of
/// `commands::ctx::Ctx`.
pub use ctx::Ctx;

/// Domain-specific command errors — see [`error::CommandError`].
///
/// Re-exported so callers can write `commands::Error::NotInRepo` etc. and
/// tests can `downcast_ref::<commands::Error>()`.
pub use error::CommandError as Error;
