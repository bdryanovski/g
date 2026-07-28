//! Personal git hooks system.
//!
//! Runs user-configured hooks alongside (not replacing) existing hook systems
//! like Husky. Hooks are configured per-repo or globally, and can filter by
//! staged files.
//!
//! ## Usage
//!
//! ```ignore
//! use crate::hooks;
//!
//! // In g commit, before git commit:
//! if !args.no_verify {
//!     hooks::run_pre_commit()?;
//! }
//!
//! // After git commit succeeds:
//! hooks::run_post_commit()?;
//! ```
//!
//! ## Configuration
//!
//! Hooks are configured in `.g/hooks.toml` (repo-local) or
//! `~/.config/g/hooks/<repo-name>.toml` (global per-repo).

mod runner;
mod staged;

use anyhow::{bail, Result};

use crate::config::hooks::HookType;
use crate::config::load_hooks;
use crate::ui::InfoBox;

pub use runner::{HookEnv, HookResult};

/// Run pre-commit hooks.
///
/// Returns an error if any hook fails (unless `no_verify` is true).
///
/// # Arguments
///
/// * `no_verify` - Skip all hooks if true (respects --no-verify flag)
pub fn run_pre_commit(no_verify: bool) -> Result<()> {
    run_hook(HookType::PreCommit, no_verify, false)
}

/// Run post-commit hooks.
///
/// Post-commit hooks are non-blocking - failures are warned but don't error.
pub fn run_post_commit(no_verify: bool) -> Result<()> {
    run_hook(HookType::PostCommit, no_verify, false)
}

/// Run pre-push hooks.
///
/// Returns an error if any hook fails (unless `no_verify` is true).
pub fn run_pre_push(no_verify: bool) -> Result<()> {
    run_hook(HookType::PrePush, no_verify, false)
}

/// Run post-checkout hooks.
///
/// Post-checkout hooks are non-blocking.
pub fn run_post_checkout() -> Result<()> {
    run_hook(HookType::PostCheckout, false, false)
}

/// Run post-merge hooks.
///
/// Post-merge hooks are non-blocking.
pub fn run_post_merge() -> Result<()> {
    run_hook(HookType::PostMerge, false, false)
}

/// Run pre-rebase hooks.
///
/// Returns an error if any hook fails.
pub fn run_pre_rebase(no_verify: bool) -> Result<()> {
    run_hook(HookType::PreRebase, no_verify, false)
}

/// Run a specific hook type.
fn run_hook(hook_type: HookType, no_verify: bool, dry_run: bool) -> Result<()> {
    // Skip if --no-verify
    if no_verify {
        return Ok(());
    }

    // Load hooks config
    let config = load_hooks()?;

    // Skip if hooks are disabled
    if !config.enabled {
        return Ok(());
    }

    // Get the hook configuration
    let hook_config = match config.get(hook_type) {
        Some(c) if c.enabled && !c.commands.is_empty() => c,
        _ => return Ok(()), // No hooks configured
    };

    // Create the hook environment
    let env = HookEnv::new(hook_type)?;

    // Run the hook
    let result = runner::run_hook(hook_config, &env, dry_run)?;

    match result {
        HookResult::Success => {
            // Success - no output needed (TaskRunner already showed it)
            Ok(())
        }
        HookResult::NoCommands | HookResult::Skipped => {
            // Nothing to do
            Ok(())
        }
        HookResult::Failed { command, message } => {
            println!();
            if hook_type.is_blocking() {
                InfoBox::danger("Hook Failed")
                    .line(&format!("{} hook '{}' failed", hook_type, command))
                    .line(&message)
                    .blank()
                    .line("Run with --no-verify to skip hooks.")
                    .print();
                bail!("Hook '{}' failed: {}", command, message);
            } else {
                // Non-blocking hook - just warn
                InfoBox::warning("Hook Warning")
                    .line(&format!("{} hook '{}' failed", hook_type, command))
                    .line(&message)
                    .print();
                Ok(())
            }
        }
    }
}
