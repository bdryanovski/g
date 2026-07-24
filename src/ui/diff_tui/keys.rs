//! Key handling for the diff TUI.
//!
//! Provides the [`Action`] enum and [`handle_key`] function that decouples
//! keystroke interpretation from state mutation and side effects.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::commands::Ctx;
use crate::diff::parse::FileDiff;
use crate::diff::reviews;

use super::state::{get_git_author, AppState};

// ─── Action enum ────────────────────────────────────────────────────────────

/// Result of processing a keypress.  The event loop inspects this to decide
/// whether to continue, quit, or perform side effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyFlow {
    /// Continue the event loop normally.
    Continue,
    /// Exit the TUI immediately.
    Quit,
    /// Refresh annotations from DB after a note was saved/deleted.
    RefreshAnnotations,
}

// ─── Main key handler ───────────────────────────────────────────────────────

/// Decode a key into a state mutation and return the resulting flow.
///
/// When the comment editor is open, most keys are redirected to it; only
/// `Esc` cancels and `Enter` commits.
pub fn handle_key(
    state: &mut AppState,
    ctx: &Ctx<'_>,
    key: KeyCode,
    mods: KeyModifiers,
    commit_hash: Option<&str>,
) -> KeyFlow {
    if state.editor_active() {
        return handle_editor_key(state, ctx, key, mods);
    }

    let shift = mods.contains(KeyModifiers::SHIFT);

    match (key, mods) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Char('q'), _) => {
            return KeyFlow::Quit;
        }
        (KeyCode::Char('?'), _) => {
            state.show_help = !state.show_help;
        }
        (KeyCode::Char('s'), _) if !shift => {
            state.layout = state.layout.cycle();
        }
        (KeyCode::Char('w'), _) => {
            state.wrap = !state.wrap;
        }
        (KeyCode::Char('t'), _) if !shift => {
            state.show_sidebar = !state.show_sidebar;
        }
        (KeyCode::Char('c'), _) if !shift => {
            // Open the inline comment editor for private note at cursor.
            let anchor = {
                let files: Vec<FileDiff> = state.files.iter().map(|(f, _)| f.clone()).collect();
                reviews::anchor_at_row(&files, state.file_idx, state.cursor)
                    .map(|raw| reviews::promote_anchor(raw, commit_hash.map(str::to_owned)))
            };
            if let Some(anchor) = anchor {
                state.open_editor_from_anchor(anchor);
            }
        }
        (KeyCode::Tab, _) => {
            let old_idx = state.file_idx;
            state.move_file(1);
            if state.file_idx != old_idx {
                // Auto-scroll sidebar to keep selected file visible
                crate::ui::diff_tui::draw::ensure_sidebar_file_visible(state, 20);
                return KeyFlow::RefreshAnnotations;
            }
        }
        (KeyCode::BackTab, _) => {
            let old_idx = state.file_idx;
            state.move_file(-1);
            if state.file_idx != old_idx {
                // Auto-scroll sidebar to keep selected file visible
                crate::ui::diff_tui::draw::ensure_sidebar_file_visible(state, 20);
                return KeyFlow::RefreshAnnotations;
            }
        }
        // Edit existing comment at cursor
        (KeyCode::Char('e'), _) if !shift => {
            if let Some(note) = state
                .inline_notes
                .get(&(state.file_idx, state.cursor))
                .cloned()
            {
                let anchor = {
                    let files: Vec<FileDiff> = state.files.iter().map(|(f, _)| f.clone()).collect();
                    reviews::anchor_at_row(&files, state.file_idx, state.cursor)
                        .map(|raw| reviews::promote_anchor(raw, commit_hash.map(str::to_owned)))
                };
                if let Some(anchor) = anchor {
                    state.open_editor_from_anchor(anchor);
                    if let Some(ref mut editor) = state.editor {
                        editor.buffer = note.body;
                        editor.cursor = editor.buffer.len();
                    }
                }
            }
        }
        // Delete comment at cursor
        (KeyCode::Char('d'), _) if !shift => {
            if state
                .annotated_rows
                .contains(&(state.file_idx, state.cursor))
            {
                let anchor = {
                    let files: Vec<FileDiff> = state.files.iter().map(|(f, _)| f.clone()).collect();
                    reviews::anchor_at_row(&files, state.file_idx, state.cursor)
                        .map(|raw| reviews::promote_anchor(raw, commit_hash.map(str::to_owned)))
                };
                if let Some(anchor) = anchor {
                    if let Some(repo_id) = ctx.repo_id {
                        let line_kind = match anchor.side {
                            reviews::CommentSide::New => crate::storage::LineKind::New,
                            reviews::CommentSide::Old => crate::storage::LineKind::Old,
                        };
                        let _ = crate::storage::reviews::delete_by_location(
                            ctx.conn,
                            repo_id,
                            &anchor.path,
                            anchor.line,
                            line_kind,
                        );
                        return KeyFlow::RefreshAnnotations;
                    }
                }
            }
        }
        (KeyCode::Char('n'), _) => state.next_hunk(true),
        (KeyCode::Char('p'), _) => state.next_hunk(false),
        (KeyCode::Char('g'), _) => state.cursor_to_edge(true),
        (KeyCode::Char('G'), _) => state.cursor_to_edge(false),
        // j/k or Down/Up move cursor; with Shift they extend selection.
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => state.move_cursor(1, shift),
        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => state.move_cursor(-1, shift),
        (KeyCode::Char('J'), KeyModifiers::SHIFT) => state.move_cursor(1, true),
        (KeyCode::Char('K'), KeyModifiers::SHIFT) => state.move_cursor(-1, true),
        (KeyCode::PageDown, _) => state.move_cursor(state.last_viewport_h as i32, false),
        (KeyCode::PageUp, _) => state.move_cursor(-(state.last_viewport_h as i32), false),
        // [ / ] scroll sidebar up/down without changing selection
        (KeyCode::Char('['), _) => {
            state.sidebar_scroll = state.sidebar_scroll.saturating_sub(1);
        }
        (KeyCode::Char(']'), _) => {
            state.sidebar_scroll = state.sidebar_scroll.saturating_add(1);
        }
        // { / } scroll sidebar by page
        (KeyCode::Char('{'), _) => {
            state.sidebar_scroll = state.sidebar_scroll.saturating_sub(10);
        }
        (KeyCode::Char('}'), _) => {
            state.sidebar_scroll = state.sidebar_scroll.saturating_add(10);
        }
        // Escape clears selection.
        (KeyCode::Esc, _) => {
            state.selection_start = None;
        }
        (KeyCode::Char(c), _) if c.is_ascii_digit() => {
            if let Some(idx) = c.to_digit(10) {
                let old_idx = state.file_idx;
                state.goto_file(idx as usize - 1);
                if state.file_idx != old_idx {
                    // Auto-scroll sidebar to keep selected file visible
                    crate::ui::diff_tui::draw::ensure_sidebar_file_visible(state, 20);
                    return KeyFlow::RefreshAnnotations;
                }
            }
        }
        _ => {}
    }
    KeyFlow::Continue
}

/// Editor-mode key handler.
fn handle_editor_key(
    state: &mut AppState,
    ctx: &Ctx<'_>,
    key: KeyCode,
    mods: KeyModifiers,
) -> KeyFlow {
    let shift = mods.contains(KeyModifiers::SHIFT);
    let alt = mods.contains(KeyModifiers::ALT);
    let ctrl = mods.contains(KeyModifiers::CONTROL);

    match key {
        KeyCode::Esc => state.cancel_editor(),
        // Ctrl+Enter or Alt+Enter inserts a newline
        KeyCode::Enter if shift || alt || ctrl => {
            if let Some(ref mut editor) = state.editor {
                editor.buffer.insert(editor.cursor, '\n');
                editor.cursor += 1;
            }
        }
        // Plain Enter saves the comment
        KeyCode::Enter => {
            if state.editor.is_some() {
                commit_editor_private(state, ctx);
                return KeyFlow::RefreshAnnotations;
            }
        }
        KeyCode::Backspace => state.editor_backspace(),
        KeyCode::Left => state.editor_move_cursor(false),
        KeyCode::Right => state.editor_move_cursor(true),
        KeyCode::Up => state.editor_move_cursor_vertical(true),
        KeyCode::Down => state.editor_move_cursor_vertical(false),
        KeyCode::Char(c) => state.editor_push_char(c),
        _ => {}
    }
    KeyFlow::Continue
}

/// Commit the editor buffer for private notes.
fn commit_editor_private(state: &mut AppState, ctx: &Ctx<'_>) {
    let Some(editor) = state.editor.take() else {
        return;
    };
    if editor.buffer.trim().is_empty() {
        return;
    }
    let Some(repo_id) = ctx.repo_id else {
        return;
    };
    let (author_name, author_email) = get_git_author();
    let _ = crate::diff::reviews::save_private_note(
        ctx.conn,
        repo_id,
        &editor.anchor,
        &editor.buffer,
        author_name.as_deref(),
        author_email.as_deref(),
    );
}
