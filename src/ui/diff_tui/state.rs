//! Diff TUI state management.
//!
//! Contains [`AppState`] and all supporting types for the diff viewer's
//! mutable state.  Separated from rendering and key handling for testability
//! and reuse.

use std::collections::{HashMap, HashSet};

use crate::diff::highlight::{self, HighlightedLine};
use crate::diff::parse::FileDiff;
use crate::diff::reviews;

// ─── Layout ─────────────────────────────────────────────────────────────────

/// TUI layout choice, cycled by the `s` key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLayout {
    /// Single column with `+`/`-` markers in the gutter.
    Stack,
    /// Visually richer single-column view with wider gutter and per-hunk
    /// spacing.
    Split,
    /// Two-column side-by-side with aligned (old | new) rows.
    Side,
}

impl DiffLayout {
    /// Pick the layout that best matches the terminal width for `auto`.
    pub fn auto(width: u16) -> Self {
        if width >= 100 {
            Self::Side
        } else {
            Self::Stack
        }
    }

    /// Next layout in the cycle: stack -> split -> side -> stack.
    pub fn cycle(self) -> Self {
        match self {
            Self::Stack => Self::Split,
            Self::Split => Self::Side,
            Self::Side => Self::Stack,
        }
    }

    /// One-word label for display in the help/status bar.
    pub fn label(self) -> &'static str {
        match self {
            Self::Stack => "stack",
            Self::Split => "split",
            Self::Side => "side",
        }
    }
}

// ─── Comment types ──────────────────────────────────────────────────────────

/// Inline comment editor state.  Held inside [`AppState::editor`] while the
/// user is composing a note.
pub struct CommentEditor {
    /// Owned anchor - `path`, `line`, `side`, optional `commit_hash`.
    pub anchor: reviews::CommentAnchor,
    /// Current buffer (body of the note).
    pub buffer: String,
    /// Byte offset of the cursor within `buffer`.
    pub cursor: usize,
}

/// Full note info for display.
#[derive(Debug, Clone)]
pub struct DisplayNote {
    pub body: String,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub line: i64,
}

// ─── App state ──────────────────────────────────────────────────────────────

/// Mutable state owned by the event loop.  Created from the parsed diff and
/// mutated by keystrokes; redrawn from scratch on each frame.
pub struct AppState {
    /// Indexed set of files in the diff (with highlighted lines).
    pub files: Vec<(FileDiff, Vec<HighlightedLine>)>,
    /// Currently-focused file (sidebar selection).
    pub file_idx: usize,
    /// Vertical scroll within the focused file, in lines.
    pub scroll: usize,
    /// Line cursor position (absolute line index in current file).
    pub cursor: usize,
    /// Active layout.
    pub layout: DiffLayout,
    /// Help overlay visibility.
    pub show_help: bool,
    /// Whether the inner content area wraps long lines.
    pub wrap: bool,
    /// Body viewport height last computed by `draw`.  Stored here so
    /// keystroke handlers can clamp scroll against the real frame size
    /// rather than a re-derived guess.
    pub last_viewport_h: usize,
    // ── Review-comment state ──────────────────────────────────────────────
    /// Optional inline comment editor.
    pub editor: Option<CommentEditor>,
    /// Set of `(file_idx, row)` pairs that already carry a private review
    /// note.
    pub annotated_rows: HashSet<(usize, usize)>,
    /// Map from `(file_idx, row)` to the full note info for inline display.
    pub inline_notes: HashMap<(usize, usize), DisplayNote>,
    /// True when `ctx.repo_id` was `Some` at startup - enables the `c` key.
    pub notes_enabled: bool,
    /// Selection anchor for multi-line comments (Shift+j/k).
    pub selection_start: Option<usize>,
    /// Whether the file tree sidebar is visible.
    pub show_sidebar: bool,
    /// Scroll offset for the sidebar file tree.
    pub sidebar_scroll: usize,
}

impl AppState {
    pub fn new(files: Vec<FileDiff>, layout: DiffLayout, wrap: bool, notes_enabled: bool) -> Self {
        let highlighted: Vec<(FileDiff, Vec<HighlightedLine>)> = files
            .into_iter()
            .map(|f| {
                let lines = highlight::engine().highlight_file(&f);
                (f, lines)
            })
            .collect();
        Self {
            files: highlighted,
            file_idx: 0,
            scroll: 0,
            cursor: 0,
            layout,
            wrap,
            show_help: false,
            last_viewport_h: 0,
            editor: None,
            annotated_rows: HashSet::new(),
            inline_notes: HashMap::new(),
            notes_enabled,
            selection_start: None,
            show_sidebar: true,
            sidebar_scroll: 0,
        }
    }

    /// Currently focused (file, highlighted-lines) pair.
    pub fn current(&self) -> Option<&(FileDiff, Vec<HighlightedLine>)> {
        self.files.get(self.file_idx)
    }

    pub fn total_lines(&self) -> usize {
        self.current().map(|(_, l)| l.len()).unwrap_or(0)
    }

    /// Move the file cursor by `delta` (wraps); resets scroll and cursor.
    pub fn move_file(&mut self, delta: i32) {
        if self.files.is_empty() {
            return;
        }
        let n = self.files.len() as i32;
        let next = (self.file_idx as i32 + delta).rem_euclid(n) as usize;
        if next != self.file_idx {
            self.file_idx = next;
            self.scroll = 0;
            self.cursor = 0;
            self.selection_start = None;
        }
    }

    /// Jump to file index `i` (out-of-range ignored); resets scroll and cursor.
    pub fn goto_file(&mut self, i: usize) {
        if i < self.files.len() && i != self.file_idx {
            self.file_idx = i;
            self.scroll = 0;
            self.cursor = 0;
            self.selection_start = None;
        }
    }

    /// Move the cursor by `delta` lines and adjust scroll to keep cursor visible.
    pub fn move_cursor(&mut self, delta: i32, extend_selection: bool) {
        let total = self.total_lines();
        if total == 0 {
            return;
        }

        if extend_selection && self.selection_start.is_none() {
            self.selection_start = Some(self.cursor);
        } else if !extend_selection {
            self.selection_start = None;
        }

        let new_cursor = (self.cursor as i32 + delta).clamp(0, (total - 1) as i32) as usize;
        self.cursor = new_cursor;

        let notes_in_view = self.count_notes_in_view();
        let note_lines = notes_in_view * 5;
        let effective_viewport = self.last_viewport_h.saturating_sub(note_lines).max(3);

        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + effective_viewport {
            self.scroll = self.cursor.saturating_sub(effective_viewport - 1);
        }
    }

    /// Count how many notes are visible in the current scroll window.
    pub fn count_notes_in_view(&self) -> usize {
        let viewport = self.last_viewport_h.max(1);
        let end = (self.scroll + viewport).min(self.total_lines());
        (self.scroll..end)
            .filter(|&row| self.inline_notes.contains_key(&(self.file_idx, row)))
            .count()
    }

    /// Jump cursor to top or bottom of the file.
    pub fn cursor_to_edge(&mut self, to_top: bool) {
        self.selection_start = None;
        let total = self.total_lines();
        if to_top {
            self.cursor = 0;
            self.scroll = 0;
        } else if total > 0 {
            self.cursor = total - 1;
            let notes_in_view = self.count_notes_in_view();
            let note_lines = notes_in_view * 5;
            let effective_viewport = self.last_viewport_h.saturating_sub(note_lines).max(3);
            self.scroll = total.saturating_sub(effective_viewport);
        }
    }

    /// Get the selection range (start, end) inclusive, or just cursor if no selection.
    pub fn selection_range(&self) -> (usize, usize) {
        match self.selection_start {
            Some(start) => (start.min(self.cursor), start.max(self.cursor)),
            None => (self.cursor, self.cursor),
        }
    }

    /// Jump to the start of the next/previous "change hunk" in the focused file.
    pub fn next_hunk(&mut self, forward: bool) {
        let Some((_, lines)) = self.current() else {
            return;
        };
        if let Some(target) = scan_hunk(self.cursor, lines, forward) {
            self.cursor = target;
            self.selection_start = None;
            let notes_in_view = self.count_notes_in_view();
            let note_lines = notes_in_view * 5;
            let effective_viewport = self.last_viewport_h.saturating_sub(note_lines).max(3);
            if self.cursor < self.scroll {
                self.scroll = self.cursor;
            } else if self.cursor >= self.scroll + effective_viewport {
                self.scroll = self.cursor.saturating_sub(effective_viewport / 2);
            }
        }
    }

    /// `true` when the inline comment editor is open.
    pub fn editor_active(&self) -> bool {
        self.editor.is_some()
    }

    /// Open the inline comment editor at the given anchor.
    pub fn open_editor_from_anchor(&mut self, anchor: reviews::CommentAnchor) {
        if !self.notes_enabled {
            return;
        }
        self.editor = Some(CommentEditor {
            anchor,
            buffer: String::new(),
            cursor: 0,
        });
    }

    /// Cancel the editor without saving.
    pub fn cancel_editor(&mut self) {
        self.editor = None;
    }

    /// Push a character into the editor buffer.
    pub fn editor_push_char(&mut self, c: char) {
        if let Some(e) = self.editor.as_mut() {
            e.buffer.insert(e.cursor, c);
            e.cursor += c.len_utf8();
        }
    }

    /// Backspace at the editor cursor.
    pub fn editor_backspace(&mut self) {
        if let Some(e) = self.editor.as_mut() {
            if e.cursor > 0 {
                let prev = e.buffer[..e.cursor]
                    .char_indices()
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                e.buffer.replace_range(prev..e.cursor, "");
                e.cursor = prev;
            }
        }
    }

    /// Move the editor cursor left / right by one grapheme.
    pub fn editor_move_cursor(&mut self, forward: bool) {
        if let Some(e) = self.editor.as_mut() {
            if forward {
                if let Some((i, c)) = e.buffer[e.cursor..].char_indices().next() {
                    e.cursor += i + c.len_utf8();
                }
            } else if e.cursor > 0 {
                let prev = e.buffer[..e.cursor]
                    .char_indices()
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                e.cursor = prev;
            }
        }
    }

    /// Move the editor cursor up / down by one line.
    pub fn editor_move_cursor_vertical(&mut self, up: bool) {
        if let Some(e) = self.editor.as_mut() {
            let before_cursor = &e.buffer[..e.cursor];
            let current_line_start = before_cursor.rfind('\n').map(|i| i + 1).unwrap_or(0);
            let current_col = e.cursor - current_line_start;

            if up {
                if current_line_start > 0 {
                    let prev_line_end = current_line_start - 1;
                    let prev_line_start = e.buffer[..prev_line_end]
                        .rfind('\n')
                        .map(|i| i + 1)
                        .unwrap_or(0);
                    let prev_line_len = prev_line_end - prev_line_start;
                    e.cursor = prev_line_start + current_col.min(prev_line_len);
                }
            } else if let Some(newline_pos) = e.buffer[e.cursor..].find('\n') {
                let next_line_start = e.cursor + newline_pos + 1;
                if next_line_start <= e.buffer.len() {
                    let next_line_end = e.buffer[next_line_start..]
                        .find('\n')
                        .map(|i| next_line_start + i)
                        .unwrap_or(e.buffer.len());
                    let next_line_len = next_line_end - next_line_start;
                    e.cursor = next_line_start + current_col.min(next_line_len);
                }
            }
        }
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Scan `lines` for the next transition from context -> change (or change -> context).
pub fn scan_hunk(from: usize, lines: &[HighlightedLine], forward: bool) -> Option<usize> {
    let is_change = |l: &HighlightedLine| matches!(l.marker, '+' | '-');
    if lines.is_empty() {
        return None;
    }
    if forward {
        let mut i = from.saturating_add(1);
        while i < lines.len() {
            let prev_change = i > 0 && is_change(&lines[i - 1]);
            let now_change = is_change(&lines[i]);
            if !prev_change && now_change {
                return Some(i);
            }
            i += 1;
        }
        None
    } else {
        let mut i = from;
        while i > 0 {
            i = i.saturating_sub(1);
            let prev_change = i > 0 && is_change(&lines[i - 1]);
            let now_change = is_change(&lines[i]);
            if !prev_change && now_change {
                return Some(i);
            }
        }
        None
    }
}

/// Get git author name and email from config.
pub fn get_git_author() -> (Option<String>, Option<String>) {
    let name = std::process::Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let email = std::process::Command::new("git")
        .args(["config", "user.email"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    (name, email)
}

/// Format a relative time string like "2 days ago", "1 year ago".
pub fn relative_time(dt: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(dt);

    let seconds = duration.num_seconds();
    if seconds < 60 {
        return "just now".to_string();
    }

    let minutes = duration.num_minutes();
    if minutes < 60 {
        return if minutes == 1 {
            "1 minute ago".to_string()
        } else {
            format!("{} minutes ago", minutes)
        };
    }

    let hours = duration.num_hours();
    if hours < 24 {
        return if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{} hours ago", hours)
        };
    }

    let days = duration.num_days();
    if days < 30 {
        return if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{} days ago", days)
        };
    }

    let months = days / 30;
    if months < 12 {
        return if months == 1 {
            "1 month ago".to_string()
        } else {
            format!("{} months ago", months)
        };
    }

    let years = days / 365;
    if years == 1 {
        "1 year ago".to_string()
    } else {
        format!("{} years ago", years)
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::parse;

    fn sample_files() -> Vec<FileDiff> {
        let raw = "\
diff --git a/x.rs b/x.rs
--- a/x.rs
+++ b/x.rs
@@ -1,3 +1,3 @@
 fn main() {
-    let x = 1;
+    let x = 2;
 }
";
        parse::parse(raw)
    }

    #[test]
    fn layout_cycle_round_trips() {
        assert_eq!(DiffLayout::Stack.cycle(), DiffLayout::Split);
        assert_eq!(DiffLayout::Split.cycle(), DiffLayout::Side);
        assert_eq!(DiffLayout::Side.cycle(), DiffLayout::Stack);
    }

    #[test]
    fn layout_auto_picks_side_for_wide_terminals() {
        assert_eq!(DiffLayout::auto(120), DiffLayout::Side);
        assert_eq!(DiffLayout::auto(60), DiffLayout::Stack);
    }

    #[test]
    fn app_state_move_file_wraps_around() {
        let mut s = AppState::new(sample_files(), DiffLayout::Stack, false, false);
        assert_eq!(s.file_idx, 0);
        s.move_file(1);
        assert_eq!(s.file_idx, 0, "single file, wraps to itself");
    }

    #[test]
    fn app_state_cursor_clamps_at_bottom() {
        let mut s = AppState::new(sample_files(), DiffLayout::Stack, false, false);
        s.last_viewport_h = 2;
        s.move_cursor(100, false);
        assert_eq!(s.cursor, 3, "cursor clamps at last line");
        s.move_cursor(-100, false);
        assert_eq!(s.cursor, 0, "cursor clamps at first line");
    }

    #[test]
    fn scan_hunk_finds_first_change_forward() {
        let files = sample_files();
        let lines = highlight::engine().highlight_file(&files[0]);
        let target = scan_hunk(0, &lines, true);
        assert_eq!(target, Some(1), "should snap to the first deletion line");
    }

    #[test]
    fn scan_hunk_backward_returns_to_hunk_start() {
        let files = sample_files();
        let lines = highlight::engine().highlight_file(&files[0]);
        let target = scan_hunk(3, &lines, false);
        assert_eq!(target, Some(1));
    }
}
