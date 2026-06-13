//! Display helpers used by the commit flow: the pre-commit staged-changes
//! summary, the per-line stat colouriser, and the type-label lookup that
//! drives the interactive picker descriptions.

use crate::commands::git as gitcmd;
use crate::ui;
use anyhow::Result;

/// Print a short diffstat summary of the currently staged changes.
pub(super) fn show_staged_summary() -> Result<()> {
    let stat = gitcmd::git_output_lossy(&["diff", "--cached", "--stat"]);
    if stat.is_empty() {
        return Ok(());
    }

    ui::print_section("Staged changes", None);
    // Show at most 12 file lines to keep the output concise.
    for line in stat.lines().take(12) {
        ui::print_indented(&colorize_stat_line(line));
    }
    if stat.lines().count() > 12 {
        ui::print_indented(&ui::muted("…and more"));
    }
    ui::print_blank();
    Ok(())
}

/// Colorise a single line from `git diff --stat`.
///
/// Turns `+` characters green and `-` characters red while keeping the rest
/// white.
fn colorize_stat_line(line: &str) -> String {
    if line.contains('|') {
        let parts: Vec<&str> = line.splitn(2, '|').collect();
        let file = parts[0];
        let rest = parts[1];
        let rest_colored = rest
            .replace('+', &ui::success("+"))
            .replace('-', &ui::danger("-"));
        format!("{}{}{}", ui::paint_text(file), ui::muted("|"), rest_colored)
    } else {
        // Summary line, e.g. "3 files changed, 42 insertions(+), 7 deletions(-)"
        ui::muted(line)
    }
}

/// Return the `(emoji_icon, description)` pair for a conventional-commit type
/// label.  Used by the type picker to render `feat — A new feature` etc.
pub(super) fn type_label_parts(t: &str) -> (&'static str, &'static str) {
    match t {
        "feat" => ("✨", "A new feature"),
        "fix" => ("🐛", "A bug fix"),
        "docs" => ("📝", "Documentation changes"),
        "style" => ("💅", "Formatting, style changes"),
        "refactor" => ("♻️ ", "Code refactoring"),
        "perf" => ("⚡", "Performance improvements"),
        "test" => ("✅", "Adding or fixing tests"),
        "build" => ("🏗️ ", "Build system changes"),
        "ci" => ("👷", "CI/CD changes"),
        "chore" => ("🔧", "Other changes (no src/test)"),
        "revert" => ("⏪", "Reverting a previous commit"),
        _ => ("·", ""),
    }
}
