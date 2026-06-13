//! Row rendering and in-place redraw helpers for inline lists.
//!
//! Inline lists print every row once, then re-render those rows in place when
//! the cursor or a checkbox changes — the scroll-buffer above is never touched.
//! [`redraw_rows`] captures that "move up N lines, clear, reprint" dance once.

use std::io::Write;

use crossterm::cursor;
use crossterm::terminal::{self, ClearType};
use crossterm::{execute, queue};

use crate::ui::interactive::SelectOption;
use crate::ui::print::{muted, paint_text, primary, success};
use crate::ui::render::indent;

/// Re-render `n` rows in place: move the cursor up to the first row, then clear
/// and reprint each via `render_row(i, writer)`. Leaves the cursor one line
/// below the last row — exactly where it started.
pub fn redraw_rows(
    n: usize,
    stdout: &mut impl Write,
    mut render_row: impl FnMut(usize, &mut dyn Write),
) {
    let _ = execute!(stdout, cursor::MoveToPreviousLine(n as u16));
    for i in 0..n {
        let _ = queue!(stdout, terminal::Clear(ClearType::CurrentLine));
        render_row(i, stdout);
    }
    let _ = stdout.flush();
}

/// Print a single-select option row (ends with `\r\n`, safe in raw mode).
pub fn option_row(opt: &SelectOption, is_cursor: bool, max_label: usize, stdout: &mut dyn Write) {
    let cursor_ch = if is_cursor { primary(">") } else { muted(" ") };
    let label = if is_cursor {
        primary(&opt.label)
    } else {
        paint_text(&opt.label)
    };
    let pad = " ".repeat(max_label.saturating_sub(opt.label.len()));
    let line = match &opt.description {
        Some(desc) if !desc.is_empty() => {
            format!(
                "{}{}  {}{}  {}\r\n",
                indent(),
                cursor_ch,
                label,
                pad,
                muted(desc)
            )
        }
        _ => format!("{}{}  {}\r\n", indent(), cursor_ch, label),
    };
    let _ = write!(stdout, "{line}");
}

/// Print a multi-select (checkbox) option row (ends with `\r\n`).
pub fn multi_row(
    opt: &SelectOption,
    is_cursor: bool,
    is_checked: bool,
    max_label: usize,
    stdout: &mut dyn Write,
) {
    let cursor_ch = if is_cursor { primary(">") } else { muted(" ") };
    let checkbox = if is_checked {
        success("[✓]")
    } else {
        muted("[ ]")
    };
    let label = if is_cursor {
        primary(&opt.label)
    } else {
        paint_text(&opt.label)
    };
    let pad = " ".repeat(max_label.saturating_sub(opt.label.len()));
    let desc = opt.description.as_deref().unwrap_or("");
    let line = if desc.is_empty() {
        format!(
            "{}{}  {}  {}{}\r\n",
            indent(),
            cursor_ch,
            checkbox,
            label,
            pad
        )
    } else {
        format!(
            "{}{}  {}  {}{}  {}\r\n",
            indent(),
            cursor_ch,
            checkbox,
            label,
            pad,
            muted(desc)
        )
    };
    let _ = write!(stdout, "{line}");
}

/// The widest label in `options`, for column alignment.
pub fn max_label_width(options: &[SelectOption]) -> usize {
    options.iter().map(|o| o.label.len()).max().unwrap_or(0)
}
