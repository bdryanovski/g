//! Full-screen diff TUI viewer.
//!
//! Componentized ratatui application for viewing diffs with syntax highlighting,
//! multiple layouts (stack / split / side-by-side), and review comment support.
//!
//! # Module layout
//!
//! ```text
//! diff_tui/
//!   mod.rs     <- this file: public entry point, event loop
//!   state.rs   <- AppState, DiffLayout, CommentEditor, and related types
//!   keys.rs    <- KeyFlow enum, handle_key() function
//!   draw.rs    <- All ratatui rendering functions
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use crate::ui::diff_tui;
//!
//! let files = diff::parse::parse(&raw_diff);
//! diff_tui::run(&files, &ctx, Some("abc123"))?;
//! ```

pub mod draw;
pub mod keys;
pub mod state;

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::Terminal;

use crate::commands::Ctx;
use crate::diff::parse::FileDiff;
use crate::diff::reviews;

pub use state::{AppState, DiffLayout};

// ─── Terminal guard ─────────────────────────────────────────────────────────

/// RAII guard that ensures terminal state is restored even on panic.
///
/// When dropped (including during unwinding), calls `ratatui::restore()`.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

// ─── Public entry point ─────────────────────────────────────────────────────

/// Open the full-screen diff viewer over `files`.
///
/// Enters the alternate screen via [`ratatui::init`] and runs the event loop
/// until the user presses `q` or `Ctrl-C`.  Terminal state is always restored
/// via [`ratatui::restore`] (including on `?` propagation).
///
/// `ctx` carries the SQLite connection and the resolved `repo_id`.  When the
/// latter is `None` (running outside a git repo) the `c` key is inert.
///
/// `commit_hash` is the SHA the diff was computed against (`Some` for staged /
/// committed diffs, `None` for unstaged working-tree diffs).
pub fn run(files: &[FileDiff], ctx: &Ctx<'_>, commit_hash: Option<&str>) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }

    let cfg = crate::config::load().unwrap_or_default();
    let initial_layout = match cfg.diff.layout.as_str() {
        "stack" => DiffLayout::Stack,
        "split" => DiffLayout::Split,
        "side" => DiffLayout::Side,
        _ => DiffLayout::auto(crate::ui::terminal_width() as u16),
    };

    let notes_enabled = ctx.repo_id.is_some();
    let mut state = AppState::new(
        files.to_vec(),
        initial_layout,
        cfg.diff.wrap_lines,
        notes_enabled,
    );

    // Pre-seed the gutter-annotation cache for the focused file.
    refresh_annotations(&mut state, ctx.conn, ctx.repo_id);

    let mut terminal = ratatui::init();
    // Guard ensures terminal is restored even if we panic during the event loop.
    let _guard = TerminalGuard;
    event_loop(&mut terminal, state, ctx, commit_hash)
    // _guard drops here, calling ratatui::restore()
}

/// Drive the ratatui event loop to completion.
fn event_loop(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    mut state: AppState,
    ctx: &Ctx<'_>,
    commit_hash: Option<&str>,
) -> Result<()> {
    let commit_hash_owned = commit_hash.map(str::to_owned);
    loop {
        let _ = terminal.draw(|f| draw::draw(f, &mut state, ctx));
        if let Ok(Event::Key(k)) = event::read() {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            match keys::handle_key(
                &mut state,
                ctx,
                k.code,
                k.modifiers,
                commit_hash_owned.as_deref(),
            ) {
                keys::KeyFlow::Continue => {}
                keys::KeyFlow::Quit => return Ok(()),
                keys::KeyFlow::RefreshAnnotations => {
                    refresh_annotations(&mut state, ctx.conn, ctx.repo_id);
                }
            }
        }
    }
}

/// Repopulate `state.annotated_rows` and `state.inline_notes` from the DB
/// for the currently-focused file.
fn refresh_annotations(state: &mut AppState, conn: &rusqlite::Connection, repo_id: Option<i64>) {
    state.annotated_rows.clear();

    state.inline_notes.clear();

    let Some(repo_id) = repo_id else {
        return;
    };

    type RowProjection = (usize, reviews::CommentSide, Option<u32>);
    let (file_idx, path, row_projections): (usize, String, Vec<RowProjection>) = {
        let Some((file, lines)) = state.current() else {
            return;
        };
        let rows = lines
            .iter()
            .enumerate()
            .map(|(row_idx, l)| {
                let side = match l.marker {
                    '+' => reviews::CommentSide::New,
                    '-' => reviews::CommentSide::Old,
                    _ => reviews::CommentSide::New,
                };
                let line_no = match side {
                    reviews::CommentSide::New => l.new_no,
                    reviews::CommentSide::Old => l.old_no,
                };
                (row_idx, side, line_no)
            })
            .collect();
        (state.file_idx, file.path.clone(), rows)
    };

    let Ok(notes) = reviews::private_notes_for_file(conn, repo_id, &path) else {
        return;
    };

    use std::collections::HashMap;
    let mut note_map: HashMap<(reviews::CommentSide, i64), crate::storage::reviews::ReviewNoteRow> =
        HashMap::new();
    for n in notes {
        let side = match n.line_kind {
            crate::storage::LineKind::New => reviews::CommentSide::New,
            crate::storage::LineKind::Old => reviews::CommentSide::Old,
        };
        note_map.insert((side, n.line), n);
    }

    for (row_idx, side, line_no) in row_projections {
        if let Some(n) = line_no {
            if let Some(note) = note_map.get(&(side, n as i64)) {
                state.annotated_rows.insert((file_idx, row_idx));
                state.inline_notes.insert(
                    (file_idx, row_idx),
                    state::DisplayNote {
                        body: note.body.clone(),
                        author_name: note.author.clone(),
                        author_email: note.author_email.clone(),
                        created_at: Some(note.created_at),
                        line: note.line,
                    },
                );
            }
        }
    }
}
