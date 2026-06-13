//! Workspace (git worktree) management.
//!
//! ## Overview
//!
//! A "workspace" is a git worktree plus a small metadata record stored in the
//! `workspaces` table of `~/.config/g/g.db`.  Git is the source of truth for
//! which worktrees exist on disk; this module adds UI metadata (friendly name,
//! description, creation timestamp).
//!
//! The store is keyed by repository ID (an auto-increment integer anchored on
//! the repo's absolute root path) so metadata from multiple repositories
//! coexists in the same database without colliding.  For repos using the
//! container layout (set by `init` or `clone --workspace`), the key is the
//! container directory's repo row — which remains stable after the directory
//! reorganisation.
//!
//! ## Folder layout
//!
//! ```text
//! workspace/
//!   mod.rs     ← this file: dispatch + clone_with_workspace re-export
//!   shared.rs  ← WorktreeInfo, ResolvedWorkspace, list_worktrees,
//!                current_repo_id, load_repo_workspaces, reconcile, …
//!   init.rs    ← `g workspace init` — reorganise into container layout
//!   clone.rs   ← `g clone --workspace <url>` — clone into container layout
//!   list.rs    ← `g workspace list`
//!   create.rs  ← `g workspace create <name>` (+ untracked-file copier)
//!   switch.rs  ← `g workspace switch [name]` — open shell in worktree
//!   delete.rs  ← `g workspace delete <name>`
//!   status.rs  ← `g workspace status`
//!   rename.rs  ← `g workspace rename <old> <new>`
//! ```

use anyhow::Result;

use crate::cli::WorkspaceCommands;
use crate::commands::Ctx;

mod clone;
mod create;
mod delete;
mod init;
mod list;
mod rename;
mod shared;
mod status;
mod switch;

// `clone_with_workspace` is called from `main::run` *before* clap parsing
// (so the `--workspace` flag can be stripped and the rest forwarded to
// `git clone`), so it is the only function besides `dispatch` that needs to
// be reachable from outside this folder.
pub use clone::run as clone_with_workspace;

// ─── Dispatch ────────────────────────────────────────────────────────────────

/// Route a parsed [`WorkspaceCommands`] subcommand to its handler.
pub fn dispatch(ctx: &Ctx, cmd: WorkspaceCommands) -> Result<()> {
    match cmd {
        WorkspaceCommands::Init => init::run(ctx),
        WorkspaceCommands::List => list::run(ctx),
        WorkspaceCommands::Create {
            name,
            branch,
            start_point,
            description,
            copy,
        } => create::run(
            ctx,
            &name,
            branch.as_deref(),
            start_point.as_deref(),
            description.as_deref(),
            copy,
        ),
        WorkspaceCommands::Switch { name } => switch::run(ctx, name.as_deref()),
        WorkspaceCommands::Delete { name, force } => delete::run(ctx, &name, force),
        WorkspaceCommands::Status => status::run(ctx),
        WorkspaceCommands::Rename { old, new } => rename::run(ctx, &old, &new),
    }
}
