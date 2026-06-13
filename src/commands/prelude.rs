//! Common imports for command modules.
//!
//! Every command file pulls roughly the same five things — `anyhow::Result`,
//! the runtime [`Ctx`](super::Ctx), `git as gitcmd` for the engine room, the
//! [`config`](crate::config) loader, and the [`ui`](crate::ui) facade.  Importing
//! them one by one in every file is line noise without information.
//!
//! ## Usage
//!
//! ```ignore
//! use crate::commands::prelude::*;
//!
//! pub(super) fn run(ctx: &Ctx, name: &str) -> Result<()> {
//!     let conn = ctx.conn;
//!     // … gitcmd::repo_root()?, ui::print_success(…), config::load()?, etc.
//! }
//! ```
//!
//! ## What's in (and what's not)
//!
//! - **In** — items used by *most* command files: error types, `Ctx`,
//!   `gitcmd`, `config`, and `ui`.
//! - **Out** — domain-specific imports (`storage::stacks as stacks_store`,
//!   `super::shared::current_repo_id`, …).  Those stay explicit so the
//!   coupling between a file and its dependencies remains visible.
//!
//! A glob import is fine here because every item is re-exported by name —
//! `cargo doc` and IDE go-to-definition still resolve to the canonical path.

// `pub(crate)` rather than `pub`: the top-level `config` and `ui` modules
// are themselves crate-private, so the re-exports must match their original
// visibility.  The prelude is only used within this crate anyway.
pub(crate) use anyhow::{bail, Context, Result};

pub(crate) use super::git as gitcmd;
pub(crate) use super::Ctx;
pub(crate) use crate::config;
pub(crate) use crate::ui;
