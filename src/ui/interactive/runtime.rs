//! Reusable event-loop scaffolding for full-screen ratatui prompts.
//!
//! Every interactive screen shares the same lifecycle: enter the alternate
//! screen, redraw on each keystroke, and restore the terminal before
//! returning.  [`run`] captures that once so individual prompts only describe
//! *what to draw* and *how a key changes state* — no boilerplate, no leaked
//! terminal state on early return.

use std::io::IsTerminal;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};

use crate::ui::render::{is_inline_prompts, is_no_interactive};

/// The result of handling one key press inside a [`run`] loop.
pub enum Flow<T> {
    /// Keep the screen open and redraw.
    Continue,
    /// Exit the loop, returning this value as the prompt's result.
    Done(T),
}

/// Return `true` when full-screen prompts are allowed.
///
/// `false` when `--no-interactive` was passed or stdin is not a TTY (piped
/// input / CI), so callers can fall back to a default without blocking.
#[inline]
pub fn is_interactive() -> bool {
    !is_no_interactive() && std::io::stdin().is_terminal()
}

/// Return `true` when prompts should render inline instead of full-screen.
#[inline]
pub fn prefers_inline() -> bool {
    is_inline_prompts()
}

/// Drive a full-screen ratatui event loop to completion.
///
/// - Enters the alternate screen via [`ratatui::init`].
/// - Each frame calls `draw(frame, &state)`.
/// - Each key **press** calls `on_key(&mut state, key)`; returning
///   [`Flow::Done`] exits and yields the value.
/// - Always calls [`ratatui::restore`] before returning, even on the `Done`
///   path, so the terminal is never left in raw/alt-screen mode.
pub fn run<S, R>(
    mut state: S,
    mut draw: impl FnMut(&mut ratatui::Frame, &S),
    mut on_key: impl FnMut(&mut S, KeyCode) -> Flow<R>,
) -> R {
    let mut terminal = ratatui::init();
    let out = loop {
        let _ = terminal.draw(|f| draw(f, &state));
        if let Ok(Event::Key(k)) = event::read() {
            if k.kind == KeyEventKind::Press {
                if let Flow::Done(result) = on_key(&mut state, k.code) {
                    break result;
                }
            }
        }
    };
    ratatui::restore();
    out
}
