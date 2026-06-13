//! Per-invocation runtime context shared by every command.
//!
//! [`Ctx`] bundles the open database connection (and any future runtime state
//! — cached config, request tracing, dry-run mode, etc.) into a single value
//! so command signatures stay stable as we evolve the shared state.
//!
//! ## Why
//!
//! Before `Ctx`, every command took `conn: &Connection` directly, which meant:
//!
//! - Adding any new shared piece of state ("the active workspace", "a cached
//!   config", "a cancellation token") required changing every signature.
//! - Tests couldn't easily inject a stub or wrap the connection.
//!
//! Now every command takes `ctx: &Ctx`. The `&Connection` is accessible via
//! `ctx.conn` so call sites and storage code don't need any plumbing changes;
//! future state can be added as fields on `Ctx` without touching signatures.
//!
//! ## Conventions
//!
//! - Public entry points (`dispatch`, `pub fn commit`, `pub fn stats`,
//!   subcommand `run` functions) take `&Ctx`.
//! - Internal helpers (`shared.rs` functions, pure computations) take
//!   `&Connection` directly — they don't benefit from a Ctx and stay easier
//!   to call from unit tests.
//! - Inside a command body, the first line is typically `let conn = ctx.conn;`
//!   so the rest of the body reads unchanged.

use rusqlite::Connection;

/// Runtime context handed to every command handler.
///
/// Owns nothing — it borrows the database connection for the duration of a
/// single command invocation.
pub struct Ctx<'a> {
    /// The open SQLite database connection.
    ///
    /// Held as a plain reference so commands can pass it to storage helpers
    /// without going through an accessor.
    pub conn: &'a Connection,
}

impl<'a> Ctx<'a> {
    /// Create a new context wrapping `conn`.
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
}
