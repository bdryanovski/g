//! Git command helpers and enhanced output modes.
//!
//! This is the "engine room" of the CLI — every interaction with the
//! underlying `git` binary lives here.
//!
//! # Folder layout
//!
//! ```text
//! git/
//!   mod.rs        ← this file: public re-exports for the rest of the crate
//!   exec.rs       ← low-level git invocation (git_output, passthrough, …)
//!   dry_run.rs    ← global dry-run flag + git_mutate / dry_run_action
//!   repo.rs       ← repo introspection (current_branch, repo_root, …)
//!   log.rs        ← `g log` (enhanced)
//!   status.rs     ← `g status` (enhanced, --porcelain=v2 parsing)
//!   add.rs        ← `g add` + interactive stager
//!   diff.rs       ← `g diff` (delta / diff-so-fancy / passthrough)
//!   branch.rs     ← `g branch` (enhanced + `branch squash`)
//!   show.rs       ← `g show` (metadata + diff)
//! ```
//!
//! ## Public surface
//!
//! Every name the rest of the crate already uses as `commands::git::XYZ` is
//! re-exported below.  Adding a new helper means importing it here too — the
//! folder split is otherwise invisible to callers.

mod add;
mod branch;
mod diff;
mod dry_run;
mod exec;
mod log;
mod repo;
mod show;
mod status;

// ── Re-exports: dry-run controls ─────────────────────────────────────────────
pub use dry_run::{
    dry_run_action, dry_run_banner, dry_run_footer, git_mutate, is_dry_run, set_dry_run,
};

// ── Re-exports: low-level git invocation ─────────────────────────────────────
pub use exec::{
    git_exe, git_output, git_output_lossy, is_ancestor, passthrough, require_clean_tree,
};

// ── Re-exports: repo introspection ───────────────────────────────────────────
pub use repo::{current_branch, default_branch, is_inside_git_repo, repo_root};

// ── Re-exports: enhanced commands ────────────────────────────────────────────
pub use add::dispatch_add;
pub use branch::{dispatch_branch, resolve_squash_message};
pub use diff::enhanced_diff;
pub use log::enhanced_log;
pub use show::enhanced_show;
pub use status::enhanced_status;
