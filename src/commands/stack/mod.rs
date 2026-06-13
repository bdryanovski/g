//! Stacked PR workflow management.
//!
//! ## Overview
//!
//! This folder implements the "Stacked Pull Requests" workflow. It tracks
//! ordered lists of branches (called *stacks*) in the `stacks` and
//! `stack_branches` tables of `~/.config/g/g.db`.
//!
//! ## Folder layout
//!
//! ```text
//! stack/
//!   mod.rs        ← this file: dispatch() + module wiring
//!   shared.rs     ← cross-subcommand helpers (current_stack, restack, GitHub token, …)
//!   new.rs        ← `g stack new <name>`
//!   add.rs        ← `g stack add <branch>`
//!   list.rs       ← `g stack list` (and the `view` alias)
//!   details.rs    ← `g stack details` — per-branch commits + live PRs
//!   switch.rs     ← `g stack switch <name>`
//!   absorb.rs     ← `g stack absorb` — merge current into the one below
//!   fold.rs       ← `g stack fold` — collapse current into parent
//!   squash.rs     ← `g stack squash` — one-commit-per-branch
//!   sync.rs       ← `g stack sync` — rebase the whole chain
//!   push.rs       ← `g stack push`
//!   pr.rs         ← `g stack pr` — create / update GitHub PRs
//!   remove.rs     ← `g stack remove <branch>`
//!   delete.rs     ← `g stack delete <name>`
//!   reorder.rs    ← `g stack up` / `g stack down`
//! ```
//!
//! Each subcommand file exposes a `pub(super) fn run(…)` so this module is
//! the only public face: command code outside this folder calls
//! [`dispatch`] and never touches a subcommand file directly.

use anyhow::Result;

use crate::cli::StackCommands;
use crate::commands::Ctx;

mod absorb;
mod add;
mod delete;
mod details;
mod fold;
mod list;
mod new;
mod pr;
mod push;
mod remove;
mod reorder;
mod shared;
mod squash;
mod switch;
mod sync;

// ─── Dispatch ────────────────────────────────────────────────────────────────

/// Route a parsed [`StackCommands`] subcommand to its handler.
///
/// Keeps every `Stack*` variant local to this module so `main::run` stays a
/// one-line `commands::stack::dispatch(&conn, cmd)?`.
pub fn dispatch(ctx: &Ctx, cmd: StackCommands) -> Result<()> {
    match cmd {
        StackCommands::New { name } => new::run(ctx, &name),
        StackCommands::Add { branch } => add::run(ctx, &branch),
        StackCommands::List => list::run(ctx),
        StackCommands::View => list::view(ctx),
        StackCommands::Details => details::run(ctx),
        StackCommands::Switch { name } => switch::run(ctx, &name),
        StackCommands::Absorb => absorb::run(ctx),
        StackCommands::Squash {
            message,
            no_interactive,
        } => squash::run(ctx, message.as_deref(), no_interactive),
        StackCommands::Fold {
            keep,
            no_interactive,
        } => fold::run(ctx, keep, no_interactive),
        StackCommands::Sync { no_interactive } => sync::run(ctx, no_interactive),
        StackCommands::Push { force } => push::run(ctx, force),
        StackCommands::Pr { open, draft } => pr::run(ctx, open, draft),
        StackCommands::Remove { branch } => remove::run(ctx, &branch),
        StackCommands::Delete { name, branches } => delete::run(ctx, &name, branches),
        StackCommands::Up => reorder::up(ctx),
        StackCommands::Down => reorder::down(ctx),
    }
}
