//! Reusable scaffolding for inline (scroll-buffer) prompts.
//!
//! Inline prompts never enter the alternate screen — they print into normal
//! terminal history and use raw mode only to read keys. This module centralises
//! the three repetitive parts: the TTY guard, the static header, and the
//! raw-mode key loop ([`run_raw`]).

use std::io::{self, IsTerminal};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal;

use crate::ui::render::{indent, is_no_interactive};

/// The result of handling one key press inside a [`run_raw`] loop.
pub enum Flow<T> {
    /// Wait for the next key.
    Continue,
    /// Exit the loop with this value.
    Done(T),
}

/// Return `true` when inline prompts may read keys (TTY + interactive).
#[inline]
pub fn is_interactive() -> bool {
    !is_no_interactive() && io::stdin().is_terminal()
}

/// A key binding hint: `(key, description)`.
pub type KeyHint<'a> = (&'a str, &'a str);

/// Print a header with colored key binding hints.
///
/// Each hint is a `(key, description)` pair. Keys are highlighted, descriptions
/// are muted, with a separator between pairs.
pub fn header_with_hints(prompt: &str, hints: &[KeyHint]) {
    use crate::ui::print::{muted, primary};

    println!();
    crate::ui::widgets::print_fieldset(prompt);
    println!();

    // Build the colored hint line
    let hint_parts: Vec<String> = hints
        .iter()
        .map(|(key, desc)| format!("{}  {}", primary(key), muted(desc)))
        .collect();

    let hint_line = hint_parts.join(&format!("   {}   ", muted("│")));
    println!("{}{}", indent(), hint_line);
    println!();
}

/// Run a raw-mode key loop, dispatching each key press to `on_key`.
///
/// The caller is responsible for printing the static header and the initial
/// body **before** calling this (cooked-mode `println!` newlines work there).
/// Raw mode is enabled for the duration and always disabled on exit.
///
/// Returns `None` only when raw mode cannot be enabled; otherwise the value
/// from the first [`Flow::Done`].
pub fn run_raw<R>(mut on_key: impl FnMut(KeyCode) -> Flow<R>) -> Option<R> {
    terminal::enable_raw_mode().ok()?;
    let out = loop {
        if let Ok(Event::Key(k)) = event::read() {
            if k.kind == KeyEventKind::Press {
                if let Flow::Done(result) = on_key(k.code) {
                    break result;
                }
            }
        }
    };
    let _ = terminal::disable_raw_mode();
    Some(out)
}
