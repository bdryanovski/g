//! Row rendering and in-place redraw helpers for inline lists.
//!
//! Inline lists print every row once, then re-render those rows in place when
//! the cursor or a checkbox changes — the scroll-buffer above is never touched.
//! [`redraw_rows`] captures that "move up N lines, clear, reprint" dance once.
//!
//! For long lists, [`ScrollView`] provides a viewport-constrained view with
//! scroll indicators showing items above/below the visible window.

use std::io::Write;

use crossterm::cursor;
use crossterm::terminal::{self, ClearType};
use crossterm::{execute, queue};

use crate::ui::interactive::SelectOption;
use crate::ui::print::{muted, paint_text, primary, success, warning};
use crate::ui::render::indent;

/// Re-render `n` rows in place: move the cursor up to the first row, then clear
/// and reprint each via `render_row(i, writer)`. Leaves the cursor one line
/// below the last row — exactly where it started.
#[allow(dead_code)]
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

// ─── Scrollable viewport ──────────────────────────────────────────────────────

/// Default maximum visible rows for scrollable lists.
const DEFAULT_MAX_VISIBLE: usize = 15;

/// Minimum visible rows (to avoid unusable tiny viewports).
const MIN_VISIBLE: usize = 5;

/// State for a scrollable list viewport.
#[derive(Clone)]
pub struct ScrollView {
    /// Total number of items in the list.
    pub total: usize,
    /// Current cursor position (0-indexed into total items).
    pub cursor: usize,
    /// First visible item index.
    pub offset: usize,
    /// Number of visible rows in the viewport.
    pub visible: usize,
}

impl ScrollView {
    /// Create a new scroll view with automatic viewport sizing.
    ///
    /// The viewport height is capped at `DEFAULT_MAX_VISIBLE` or terminal height - 6
    /// (leaving room for header, hints, and scroll indicators).
    pub fn new(total: usize) -> Self {
        let term_height = console::Term::stdout().size().0 as usize;
        // Reserve lines for: header (2) + hint (2) + scroll indicators (2)
        let available = term_height.saturating_sub(6);
        let visible = available.clamp(MIN_VISIBLE, DEFAULT_MAX_VISIBLE).min(total);

        Self {
            total,
            cursor: 0,
            offset: 0,
            visible,
        }
    }

    /// Create a scroll view with explicit max visible rows.
    #[allow(dead_code)]
    pub fn with_max_visible(total: usize, max_visible: usize) -> Self {
        let visible = max_visible.clamp(MIN_VISIBLE, total);
        Self {
            total,
            cursor: 0,
            offset: 0,
            visible,
        }
    }

    /// Move cursor down, adjusting viewport offset if needed.
    pub fn move_down(&mut self) {
        if self.cursor < self.total - 1 {
            self.cursor += 1;
            // If cursor moves below visible window, scroll down
            if self.cursor >= self.offset + self.visible {
                self.offset = self.cursor - self.visible + 1;
            }
        }
    }

    /// Move cursor up, adjusting viewport offset if needed.
    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            // If cursor moves above visible window, scroll up
            if self.cursor < self.offset {
                self.offset = self.cursor;
            }
        }
    }

    /// Jump to start of list.
    #[allow(dead_code)]
    pub fn move_to_start(&mut self) {
        self.cursor = 0;
        self.offset = 0;
    }

    /// Jump to end of list.
    #[allow(dead_code)]
    pub fn move_to_end(&mut self) {
        self.cursor = self.total.saturating_sub(1);
        self.offset = self.total.saturating_sub(self.visible);
    }

    /// Range of visible items `[start, end)`.
    pub fn visible_range(&self) -> std::ops::Range<usize> {
        let end = (self.offset + self.visible).min(self.total);
        self.offset..end
    }

    /// Cursor position relative to the visible window (for highlighting).
    #[allow(dead_code)]
    pub fn local_cursor(&self) -> usize {
        self.cursor - self.offset
    }

    /// Returns true if there are items above the visible window.
    pub fn has_items_above(&self) -> bool {
        self.offset > 0
    }

    /// Returns true if there are items below the visible window.
    pub fn has_items_below(&self) -> bool {
        self.offset + self.visible < self.total
    }

    /// Number of items above the visible window.
    pub fn items_above(&self) -> usize {
        self.offset
    }

    /// Number of items below the visible window.
    pub fn items_below(&self) -> usize {
        self.total.saturating_sub(self.offset + self.visible)
    }

    /// Total number of lines this view will render (visible + indicators).
    #[allow(dead_code)]
    pub fn rendered_lines(&self) -> usize {
        let mut lines = self.visible.min(self.total);
        if self.has_items_above() {
            lines += 1;
        }
        if self.has_items_below() {
            lines += 1;
        }
        lines
    }
}

/// Print the "more items above" indicator.
pub fn scroll_indicator_above(count: usize, stdout: &mut dyn Write) {
    let msg = if count == 1 {
        "↑ 1 more item above".to_string()
    } else {
        format!("↑ {} more items above", count)
    };
    let _ = write!(stdout, "{}   {}\r\n", indent(), muted(&msg));
}

/// Print the "more items below" indicator.
pub fn scroll_indicator_below(count: usize, stdout: &mut dyn Write) {
    let msg = if count == 1 {
        "↓ 1 more item below".to_string()
    } else {
        format!("↓ {} more items below", count)
    };
    let _ = write!(stdout, "{}   {}\r\n", indent(), muted(&msg));
}

/// Print scroll position summary (e.g., "Showing 1-15 of 42").
#[allow(dead_code)]
pub fn scroll_position_summary(view: &ScrollView, selected_count: usize, stdout: &mut dyn Write) {
    let start = view.offset + 1;
    let end = (view.offset + view.visible).min(view.total);
    let pos = format!("Showing {}-{} of {}", start, end, view.total);
    let selected = if selected_count > 0 {
        format!("  •  {} selected", selected_count)
    } else {
        String::new()
    };
    let _ = write!(
        stdout,
        "{}   {}{}\r\n",
        indent(),
        muted(&pos),
        warning(&selected)
    );
}

/// Re-render a scrollable view in place.
///
/// This handles moving the cursor back, clearing lines, and re-rendering
/// the visible window with scroll indicators.
pub fn redraw_scrollable<F>(
    view: &ScrollView,
    prev_lines: usize,
    stdout: &mut impl Write,
    mut render_row: F,
) -> usize
where
    F: FnMut(usize, bool, &mut dyn Write),
{
    // Move back to the start of the previously rendered content
    if prev_lines > 0 {
        let _ = execute!(stdout, cursor::MoveToPreviousLine(prev_lines as u16));
    }

    let mut lines_rendered = 0;

    // Render "items above" indicator
    if view.has_items_above() {
        let _ = queue!(stdout, terminal::Clear(ClearType::CurrentLine));
        scroll_indicator_above(view.items_above(), stdout);
        lines_rendered += 1;
    }

    // Render visible items
    let range = view.visible_range();
    for i in range {
        let _ = queue!(stdout, terminal::Clear(ClearType::CurrentLine));
        let is_cursor = i == view.cursor;
        render_row(i, is_cursor, stdout);
        lines_rendered += 1;
    }

    // Render "items below" indicator
    if view.has_items_below() {
        let _ = queue!(stdout, terminal::Clear(ClearType::CurrentLine));
        scroll_indicator_below(view.items_below(), stdout);
        lines_rendered += 1;
    }

    // Clear any leftover lines from previous render
    for _ in lines_rendered..prev_lines {
        let _ = queue!(stdout, terminal::Clear(ClearType::CurrentLine));
        let _ = write!(stdout, "\r\n");
    }

    let _ = stdout.flush();
    lines_rendered
}
