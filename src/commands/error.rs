//! Domain-specific errors raised by command handlers.
//!
//! Most commands previously returned `anyhow::bail!("…")` with the message
//! baked into a string.  That worked but pushed the entire failure contract
//! into prose, so:
//!
//! - Tests had to match on the message text — fragile and noisy.
//! - The UI couldn't distinguish "user typed an unknown stack name" from
//!   "the database is corrupt" — every error was just text.
//! - Adding or rephrasing a hint meant chasing the same string through every
//!   call site.
//!
//! `CommandError` fixes that.  Each variant captures one *kind* of expected
//! failure, with structured fields where the message varies (the name the
//! user typed, etc.).  Tests can downcast and pattern-match; the UI can
//! render kind-specific hints; rewording the canonical message means editing
//! one `#[error(…)]` attribute.
//!
//! ## Composition with `anyhow`
//!
//! `CommandError` implements `std::error::Error` (via `thiserror`), so every
//! variant participates in the existing `Result<()> = anyhow::Result<()>`
//! chain via `?`.  Call sites can still attach contextual hints with
//! `.with_context(…)` — the typed error supplies the *kind*, the context
//! adds the *hint*:
//!
//! ```ignore
//! stacks
//!     .iter()
//!     .find(|s| s.name == name)
//!     .ok_or_else(|| CommandError::StackNotFound(name.to_string()))
//!     .with_context(|| format!("Run `{} stack list` to see all stacks.", bin_name()))?;
//! ```
//!
//! ## When to use a variant vs. plain `bail!`
//!
//! - Recurring, **expected** failures (not-in-repo, workspace-not-found,
//!   stack-already-exists, …) → `CommandError`.
//! - One-off contextual errors that won't be tested or branched on (e.g.
//!   "Failed to spawn shell '{shell}'") → plain `anyhow::bail!` is fine.

use thiserror::Error;

/// Expected failure conditions raised by command handlers.
#[derive(Debug, Error)]
pub enum CommandError {
    // ── Repo / git state ───────────────────────────────────────────────────
    /// The current working directory is not inside a git repository.
    #[error("Not inside a git repository.")]
    NotInRepo,

    /// `HEAD` is detached — operations that need a branch name can't proceed.
    #[error("Detached HEAD; checkout a branch first.")]
    DetachedHead,

    // ── Stacks ─────────────────────────────────────────────────────────────
    /// No stacks exist in the current repository.
    #[error("No stacks in this repository.")]
    NoStacks,

    /// A stack with this name already exists in the current repository.
    #[error("Stack '{0}' already exists in this repository.")]
    StackExists(String),

    /// No stack matched the user-supplied identifier.
    #[error("Stack '{0}' not found.")]
    StackNotFound(String),

    /// The branch is not registered in any stack.
    #[error("Branch '{0}' is not part of any stack.")]
    BranchNotInStack(String),

    /// The branch was expected to be a member of the named stack but isn't.
    #[error("Branch '{0}' not found in stack")]
    BranchMissingFromStack(String),

    // ── Workspaces ─────────────────────────────────────────────────────────
    /// A workspace with this name already exists in the current repository.
    #[error("Workspace '{0}' already exists. Use a different name.")]
    WorkspaceExists(String),

    /// No workspace matched the user-supplied identifier.
    #[error("Workspace '{0}' not found.")]
    WorkspaceNotFound(String),
}
