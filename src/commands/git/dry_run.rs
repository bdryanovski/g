//! Dry-run mode: an atomic flag that, when set, prints the git commands and
//! side-effects that *would* run instead of executing them.
//!
//! [`git_mutate`] is the canonical entry point — every mutating git call in
//! the codebase goes through it, so dry-run is a single-flag toggle for the
//! entire CLI.

use anyhow::Result;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::ui;

use super::exec::git_output;

// `static` variables live for the entire lifetime of the program.
// `AtomicBool` / `AtomicUsize` are safe to access from multiple threads
// without a `Mutex` because they use hardware-level atomic operations.
static DRY_RUN: AtomicBool = AtomicBool::new(false);
static DRY_RUN_STEP: AtomicUsize = AtomicUsize::new(0);

/// Enable or disable dry-run mode and reset the step counter.
pub fn set_dry_run(enabled: bool) {
    DRY_RUN.store(enabled, Ordering::SeqCst);
    DRY_RUN_STEP.store(0, Ordering::SeqCst);
}

/// Returns `true` when dry-run mode is active.
pub fn is_dry_run() -> bool {
    DRY_RUN.load(Ordering::SeqCst)
}

/// Returns the number of mutating steps logged so far in dry-run mode.
pub fn step_count() -> usize {
    DRY_RUN_STEP.load(Ordering::SeqCst)
}

/// Increment the step counter and return the new value.
fn next_step() -> usize {
    DRY_RUN_STEP.fetch_add(1, Ordering::SeqCst) + 1
}

/// Execute a mutating git command, or print the planned command in dry-run mode.
///
/// Use this for any `git` invocation that writes to the repository (checkout,
/// commit, rebase, …).  Read-only calls (log, rev-parse, …) should use
/// [`super::exec::git_output`] directly.
pub fn git_mutate(args: &[&str], explanation: &str) -> Result<String> {
    if is_dry_run() {
        print_dry_run_git(args, explanation);
        return Ok(String::new());
    }
    git_output(args)
}

/// Log a non-git side effect (file write, API call, …) in dry-run mode.
///
/// In normal (non-dry-run) mode this is a no-op.
pub fn dry_run_action(action: &str, explanation: &str) {
    if is_dry_run() {
        let step = next_step();
        let label = format!("Step {}", step);
        ui::print_blank();
        ui::print_indented(&format!(
            "{} {} {}",
            ui::primary_bold(&label),
            ui::muted("▸"),
            ui::warning(action)
        ));
        ui::print_indented(&format!(
            "{}  {}",
            " ".repeat(label.len()),
            ui::muted(explanation)
        ));
    }
}

/// Print a dry-run step for a git command.
///
/// Delegates to [`dry_run_action`] after formatting the command as
/// `"git <args>"`.
pub(super) fn print_dry_run_git(args: &[&str], explanation: &str) {
    dry_run_action(&format!("git {}", args.join(" ")), explanation);
}

/// Print the dry-run banner shown at the start of a `--dry-run` invocation.
pub fn dry_run_banner() {
    ui::print_blank();
    ui::print_fieldset("⚡  Dry Run — preview only, no changes will be made");
}

/// Print the dry-run footer summarising how many operations would have run.
pub fn dry_run_footer() {
    let steps = step_count();
    ui::print_blank();
    if steps > 0 {
        ui::print_fieldset(&format!(
            "{}  {} would be performed — re-run without --dry-run to execute",
            steps,
            if steps == 1 {
                "operation"
            } else {
                "operations"
            }
        ));
    } else {
        ui::print_fieldset("No mutating operations to preview");
    }
    ui::print_blank();
}
