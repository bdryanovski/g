//! Mode 1 — styled print helpers and semantic styling API.
//!
//! All functions here produce output to **stdout** (or stderr for warnings
//! and errors).  Colors come exclusively from [`super::theme::current()`] so
//! they change automatically when the active theme changes.
//!
//! # Semantic styling API
//!
//! Instead of calling `"text".cyan().bold()` (which hardcodes a color),
//! command files call `ui::primary_bold("text")`.  The mapping from semantic
//! role to actual color lives only in [`super::theme::Palette`].
//!
//! # Output helpers
//!
//! `print_info`, `print_success`, etc. are the standard one-liner output
//! functions.  `print_line` and `print_indented` are the low-level primitives
//! for any output that doesn't fit a named helper — they ensure all output
//! still routes through this module rather than scattered raw `println!` calls.

use super::render::{paint, paint_bold, paint_bold_underline, paint_dim, paint_underline, INDENT};
use super::theme;

// ─── Generic output primitives ───────────────────────────────────────────────

/// Print `content` as a complete line to stdout.
///
/// Use this anywhere a raw `println!("{}", ...)` would appear in command files.
/// The content is expected to already contain any ANSI styling from the
/// semantic helpers below.
#[allow(dead_code)]
pub fn print_line(content: &str) {
    println!("{}", content);
}

/// Print `content` with the standard left-margin indent as a complete line.
///
/// Equivalent to `println!("  {}", content)`.  Use this to route any
/// `println!("  {}", ...)` calls in command files through the ui module.
#[allow(dead_code)]
pub fn print_indented(content: &str) {
    println!("{}{}", INDENT, content);
}

// ─── Semantic styling helpers ─────────────────────────────────────────────────
//
// Every call reads its color from `theme::current()` so swapping themes
// automatically changes all styled output.  Use these instead of raw color
// names anywhere in command files.

/// Render `text` in the primary accent color (cyan by default).
pub fn primary(text: &str) -> String {
    paint(text, theme::current().palette.primary)
}

/// Render `text` in primary color, bold.
pub fn primary_bold(text: &str) -> String {
    paint_bold(text, theme::current().palette.primary)
}

/// Render `text` in the success color (green by default).
pub fn success(text: &str) -> String {
    paint(text, theme::current().palette.success)
}

/// Render `text` in success color, bold.
pub fn success_bold(text: &str) -> String {
    paint_bold(text, theme::current().palette.success)
}

/// Render `text` in the warning color (yellow by default).
pub fn warning(text: &str) -> String {
    paint(text, theme::current().palette.warning)
}

/// Render `text` in warning color, bold.
pub fn warning_bold(text: &str) -> String {
    paint_bold(text, theme::current().palette.warning)
}

/// Render `text` in the danger color (red by default).
pub fn danger(text: &str) -> String {
    paint(text, theme::current().palette.danger)
}

/// Render `text` in danger color, bold.
pub fn danger_bold(text: &str) -> String {
    paint_bold(text, theme::current().palette.danger)
}

/// Render `text` in the muted color (dark gray by default).
pub fn muted(text: &str) -> String {
    paint(text, theme::current().palette.muted)
}

/// Render `text` in muted color, bold.
pub fn muted_bold(text: &str) -> String {
    paint_bold(text, theme::current().palette.muted)
}

/// Render `text` in the accent color (magenta by default).
#[allow(dead_code)]
pub fn accent(text: &str) -> String {
    paint(text, theme::current().palette.accent)
}

/// Render `text` in body-text color (white by default), bold.
pub fn text_bold(text: &str) -> String {
    paint_bold(text, theme::current().palette.text)
}

/// Render `text` in body-text color.
pub fn paint_text(text: &str) -> String {
    paint(text, theme::current().palette.text)
}

/// Render `text` as dim (low-intensity, muted color).
pub fn dimmed(text: &str) -> String {
    paint_dim(text, theme::current().palette.muted)
}

/// Render `text` underlined in the primary color.
#[allow(dead_code)]
pub fn link(text: &str) -> String {
    paint_underline(text, theme::current().palette.primary)
}

/// Render `text` underlined in the muted color.
pub fn link_muted(text: &str) -> String {
    paint_underline(text, theme::current().palette.muted)
}

/// Render `text` bold + underlined in the primary color — for paths / URLs.
pub fn link_primary_bold(text: &str) -> String {
    paint_bold_underline(text, theme::current().palette.primary)
}

// ─── Status-message helpers ──────────────────────────────────────────────────

/// Print `  ℹ  <msg>` to stdout.
pub fn print_info(msg: &str) {
    let t = theme::current();
    println!(
        "{} {} {}",
        INDENT,
        paint(t.icons.info, t.palette.primary),
        msg
    );
}

/// Print `  ✓  <msg>` to stdout.
pub fn print_success(msg: &str) {
    let t = theme::current();
    println!(
        "{} {} {}",
        INDENT,
        paint_bold(t.icons.success, t.palette.success),
        msg
    );
}

/// Print `  ⚠  <msg>` to **stderr**.
pub fn print_warning(msg: &str) {
    let t = theme::current();
    eprintln!(
        "{} {} {}",
        INDENT,
        paint_bold(t.icons.warning, t.palette.warning),
        msg
    );
}

/// Print `  ✗  <msg>` to **stderr**.
pub fn print_error(msg: &str) {
    let t = theme::current();
    eprintln!(
        "{} {} {}",
        INDENT,
        paint_bold(t.icons.error, t.palette.danger),
        msg
    );
}

/// Print a dim tip hint: `  tip:  <msg>` to stdout.
pub fn print_tip(msg: &str) {
    println!("{} {}  {}", INDENT, muted_bold("tip:"), muted(msg));
}

/// Print a blank line to stdout.
pub fn print_blank() {
    println!();
}

/// Print a full-width `───` rule scaled to the terminal width.
#[allow(dead_code)]
pub fn print_rule() {
    let width = super::render::terminal_width()
        .saturating_sub(INDENT.len())
        .max(10);
    println!("{}{}", INDENT, muted(&"─".repeat(width)));
}

/// Print a numbered step: `  [n/total]  <msg>`.
pub fn print_step(step: usize, total: usize, msg: &str) {
    println!(
        "{}{} {}",
        INDENT,
        muted_bold(&format!("[{}/{}]", step, total)),
        msg
    );
}

/// Print a subsection header with an optional item count.
///
/// Outputs a blank line, a bold white title (with optional `(n)` count),
/// and a `───` rule beneath it.
///
/// ```text
///
///   Staged Changes (2)
///   ─────────────────────────────────────────────────
/// ```
pub fn print_section(title: &str, count: Option<usize>) {
    println!();
    if let Some(n) = count {
        println!(
            "{} {} {}",
            INDENT,
            text_bold(title),
            muted(&format!("({})", n))
        );
    } else {
        println!("{} {}", INDENT, text_bold(title));
    }
    let width = super::render::terminal_width()
        .saturating_sub(INDENT.len() + 1)
        .max(10);
    println!("{} {}", INDENT, muted(&"─".repeat(width)));
}

/// Print aligned key-value pairs, with keys in muted color.
///
/// Values are printed as-is (ANSI codes in value strings are respected).
pub fn print_key_value_pairs(pairs: &[(&str, String)]) {
    let max_key = pairs.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (key, val) in pairs {
        let padding = " ".repeat(max_key - key.len());
        println!(
            "{}{}{} {}  {}",
            INDENT,
            muted(key),
            padding,
            muted(" "),
            val
        );
    }
}

/// Print the `verb stack: <name>` banner used by push / sync / pr operations.
pub fn print_stack_banner(verb: &str, stack_name: &str) {
    println!();
    println!(
        "{}  {} {}",
        INDENT,
        text_bold(verb),
        primary_bold(stack_name)
    );
    println!();
}
