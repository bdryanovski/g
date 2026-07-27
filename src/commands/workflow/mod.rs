//! Git workflow management commands.
//!
//! ## Overview
//!
//! This module implements customizable git workflows, allowing users to define
//! and use branching strategies like Git Flow, GitHub Flow, trunk-based, or
//! create their own custom workflows.
//!
//! ## Folder layout
//!
//! ```text
//! workflow/
//!   mod.rs        <- this file: dispatch() + module wiring
//!   shared.rs     <- cross-subcommand helpers (load workflow, resolve branch type, etc.)
//!   hooks.rs      <- hook execution engine
//!   start.rs      <- `g workflow start <type> <name>`
//!   finish.rs     <- `g workflow finish`
//!   sync.rs       <- `g workflow sync`
//!   publish.rs    <- `g workflow publish`
//!   status.rs     <- `g workflow status`
//!   list.rs       <- `g workflow list`
//!   info.rs       <- `g workflow info <name>`
//!   use_cmd.rs    <- `g workflow use <name>`
//!   create.rs     <- `g workflow create`
//!   edit.rs       <- `g workflow edit`
//!   init.rs       <- `g workflow init`
//!   validate.rs   <- `g workflow validate`
//!   clone.rs      <- `g workflow clone`
//!   export.rs     <- `g workflow export`
//!   import.rs     <- `g workflow import`
//! ```

use anyhow::Result;

use crate::cli::WorkflowCommands;
use crate::commands::Ctx;

mod clone;
mod create;
mod edit;
mod export;
mod finish;
mod hooks;
mod import;
mod info;
mod init;
mod list;
mod publish;
mod shared;
mod start;
mod status;
mod sync;
mod use_cmd;
mod validate;

// ─── Dispatch ────────────────────────────────────────────────────────────────

/// Route a parsed [`WorkflowCommands`] subcommand to its handler.
pub fn dispatch(ctx: &Ctx, cmd: WorkflowCommands) -> Result<()> {
    match cmd {
        // Lifecycle
        WorkflowCommands::Start(args) => start::run(ctx, args),
        WorkflowCommands::Finish(args) => finish::run(ctx, args),
        WorkflowCommands::Sync(args) => sync::run(ctx, args),
        WorkflowCommands::Publish(args) => publish::run(ctx, args),

        // Status and information
        WorkflowCommands::Status => status::run(ctx),
        WorkflowCommands::List => list::run(ctx),
        WorkflowCommands::Info(args) => info::run(ctx, args),
        WorkflowCommands::Use(args) => use_cmd::run(ctx, args),

        // Configuration
        WorkflowCommands::Create(args) => create::run(ctx, args),
        WorkflowCommands::Edit(args) => edit::run(ctx, args),
        WorkflowCommands::Init(args) => init::run(ctx, args),
        WorkflowCommands::Validate(args) => validate::run(ctx, args),

        // Sharing
        WorkflowCommands::Clone(args) => clone::run(ctx, args),
        WorkflowCommands::Export(args) => export::run(ctx, args),
        WorkflowCommands::Import(args) => import::run(ctx, args),
    }
}
