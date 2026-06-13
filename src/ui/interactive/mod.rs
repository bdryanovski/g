//! Mode 3 — full-screen ratatui TUI prompts.
//!
//! This folder is the **reusable interactive kit**. The heavy lifting lives in:
//!
//! - [`runtime`] — the enter/draw/key/restore event loop ([`runtime::run`])
//!   and the TTY guards.
//! - [`layout`] — shared vertical zone splits.
//! - [`widgets`] — themed header, help bar, cursor list, input line, paginator,
//!   and the composed [`widgets::scroll_list`].
//!
//! Each public prompt below is now just *state + a draw call + a key match*, so
//! adding a new screen is a few lines and the business logic stays obvious.
//!
//! Every entry point first honours inline mode ([`runtime::prefers_inline`])
//! and the non-TTY guard ([`runtime::is_interactive`]), falling back to a
//! sensible default so scripts never block.

mod layout;
mod runtime;
mod widgets;

use crossterm::event::KeyCode;
use ratatui::layout::Constraint::{Length, Min};

use runtime::{is_interactive, prefers_inline, run, Flow};

// ─── Public data type ─────────────────────────────────────────────────────────

/// A selectable item: a label plus an optional muted description column.
#[allow(dead_code)]
pub struct SelectOption {
    /// Primary label shown in the list.
    pub label: String,
    /// Optional description rendered in muted color after the label.
    pub description: Option<String>,
}

impl SelectOption {
    /// Create a label-only option.
    #[allow(dead_code)]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            description: None,
        }
    }

    /// Create an option with a description column.
    #[allow(dead_code)]
    pub fn with_description(label: impl Into<String>, desc: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            description: Some(desc.into()),
        }
    }
}

// ─── select ───────────────────────────────────────────────────────────────────

/// Full-screen single-choice list picker. Returns the chosen index, or `None`
/// on cancel / non-interactive.
pub fn select(prompt: &str, options: &[SelectOption]) -> Option<usize> {
    if prefers_inline() {
        return super::inline::inline_select(prompt, options);
    }
    let n = options.len();
    if n == 0 || !is_interactive() {
        return None;
    }
    run(
        0usize,
        |f, &cursor| {
            let area = f.area();
            widgets::scroll_list(
                f,
                area,
                prompt,
                options,
                cursor,
                None,
                &[("j/k", "move"), ("Enter", "select"), ("q", "quit")],
            );
        },
        |cursor, key| match key {
            KeyCode::Char('j') | KeyCode::Down => {
                *cursor = (*cursor + 1).min(n - 1);
                Flow::Continue
            }
            KeyCode::Char('k') | KeyCode::Up => {
                *cursor = cursor.saturating_sub(1);
                Flow::Continue
            }
            KeyCode::Enter => Flow::Done(Some(*cursor)),
            KeyCode::Esc | KeyCode::Char('q') => Flow::Done(None),
            _ => Flow::Continue,
        },
    )
}

// ─── multi_select ─────────────────────────────────────────────────────────────

/// State for a checkbox list.
struct MultiState {
    cursor: usize,
    checked: Vec<bool>,
}

/// Full-screen checkbox list. Returns indices of checked items (empty on cancel).
pub fn multi_select(prompt: &str, options: &[SelectOption]) -> Vec<usize> {
    if prefers_inline() {
        return super::inline::inline_multi_select(prompt, options, &[]);
    }
    let n = options.len();
    if n == 0 || !is_interactive() {
        return vec![];
    }
    run(
        MultiState {
            cursor: 0,
            checked: vec![false; n],
        },
        |f, st| {
            let area = f.area();
            widgets::scroll_list(
                f,
                area,
                prompt,
                options,
                st.cursor,
                Some(&st.checked),
                &[
                    ("Space", "toggle"),
                    ("a", "all/none"),
                    ("Enter", "confirm"),
                    ("q", "cancel"),
                ],
            );
        },
        |st, key| match key {
            KeyCode::Char('j') | KeyCode::Down => {
                st.cursor = (st.cursor + 1).min(n - 1);
                Flow::Continue
            }
            KeyCode::Char('k') | KeyCode::Up => {
                st.cursor = st.cursor.saturating_sub(1);
                Flow::Continue
            }
            KeyCode::Char(' ') => {
                st.checked[st.cursor] = !st.checked[st.cursor];
                Flow::Continue
            }
            KeyCode::Char('a') => {
                let all = st.checked.iter().all(|&c| c);
                st.checked.iter_mut().for_each(|c| *c = !all);
                Flow::Continue
            }
            KeyCode::Enter => Flow::Done(
                st.checked
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &c)| c.then_some(i))
                    .collect(),
            ),
            KeyCode::Esc | KeyCode::Char('q') => Flow::Done(vec![]),
            _ => Flow::Continue,
        },
    )
}

// ─── input ────────────────────────────────────────────────────────────────────

/// Single-line text input. Returns `None` on cancel.
pub fn input(prompt: &str, default: Option<&str>) -> Option<String> {
    input_validated(prompt, default, |_| Ok(()))
}

/// State for a text input.
struct InputState {
    chars: Vec<char>,
    cursor: usize,
    error: Option<String>,
}

/// Text input with a validation closure run on `Enter` (an `Err` keeps the
/// prompt open and shows the message). Returns `None` on cancel.
pub fn input_validated<F>(prompt: &str, default: Option<&str>, validate: F) -> Option<String>
where
    F: Fn(&str) -> Result<(), String>,
{
    if prefers_inline() {
        return super::inline::inline_input_validated(prompt, default, validate);
    }
    if !is_interactive() {
        return default.map(str::to_owned);
    }
    let chars: Vec<char> = default.unwrap_or("").chars().collect();
    let cursor = chars.len();
    run(
        InputState {
            chars,
            cursor,
            error: None,
        },
        |f, st| {
            let area = f.area();
            let z = layout::rows(
                area,
                [
                    Length(1),
                    Length(1),
                    Length(1),
                    Length(1),
                    Min(0),
                    Length(1),
                ],
            );
            widgets::header(f, prompt, z[0]);

            let before: String = st.chars[..st.cursor].iter().collect();
            let cursor_ch = st
                .chars
                .get(st.cursor)
                .map(|c| c.to_string())
                .unwrap_or_else(|| " ".to_string());
            let after: String = if st.cursor < st.chars.len() {
                st.chars[st.cursor + 1..].iter().collect()
            } else {
                String::new()
            };
            widgets::input_line(f, &before, &cursor_ch, &after, z[2]);

            if let Some(msg) = &st.error {
                widgets::error_line(f, msg, z[3]);
            }
            widgets::help(f, &[("Enter", "confirm"), ("Esc", "cancel")], z[5]);
        },
        |st, key| {
            st.error = None;
            match key {
                KeyCode::Char(c) => {
                    st.chars.insert(st.cursor, c);
                    st.cursor += 1;
                    Flow::Continue
                }
                KeyCode::Backspace => {
                    if st.cursor > 0 {
                        st.chars.remove(st.cursor - 1);
                        st.cursor -= 1;
                    }
                    Flow::Continue
                }
                KeyCode::Delete => {
                    if st.cursor < st.chars.len() {
                        st.chars.remove(st.cursor);
                    }
                    Flow::Continue
                }
                KeyCode::Left => {
                    st.cursor = st.cursor.saturating_sub(1);
                    Flow::Continue
                }
                KeyCode::Right => {
                    st.cursor = (st.cursor + 1).min(st.chars.len());
                    Flow::Continue
                }
                KeyCode::Home => {
                    st.cursor = 0;
                    Flow::Continue
                }
                KeyCode::End => {
                    st.cursor = st.chars.len();
                    Flow::Continue
                }
                KeyCode::Enter => {
                    let value: String = st.chars.iter().collect();
                    match validate(&value) {
                        Ok(()) => Flow::Done(Some(value)),
                        Err(msg) => {
                            st.error = Some(msg);
                            Flow::Continue
                        }
                    }
                }
                KeyCode::Esc => Flow::Done(None),
                _ => Flow::Continue,
            }
        },
    )
}

// ─── confirm ─────────────────────────────────────────────────────────────────

/// Full-screen yes/no prompt. Returns `default` when non-interactive.
pub fn confirm(prompt: &str, default: bool) -> bool {
    if prefers_inline() {
        return super::inline::inline_confirm(prompt, default);
    }
    if !is_interactive() {
        return default;
    }
    run(
        default,
        |f, &choice| {
            let area = f.area();
            let z = layout::rows(area, [Length(1), Length(1), Length(1), Min(0), Length(1)]);
            widgets::header(f, prompt, z[0]);
            widgets::yes_no(f, choice, z[2]);
            widgets::help(
                f,
                &[
                    ("y/n", "choose"),
                    ("←/→", "toggle"),
                    ("Enter", "confirm"),
                    ("Esc", "cancel"),
                ],
                z[4],
            );
        },
        |choice, key| match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => Flow::Done(true),
            KeyCode::Char('n') | KeyCode::Char('N') => Flow::Done(false),
            KeyCode::Left | KeyCode::Char('h') => {
                *choice = true;
                Flow::Continue
            }
            KeyCode::Right | KeyCode::Char('l') => {
                *choice = false;
                Flow::Continue
            }
            KeyCode::Enter => Flow::Done(*choice),
            KeyCode::Esc => Flow::Done(default),
            _ => Flow::Continue,
        },
    )
}

// ─── fuzzy_select ─────────────────────────────────────────────────────────────

/// State for the fuzzy picker.
struct FuzzyState {
    query: Vec<char>,
    cursor: usize,
}

/// Case-insensitive substring filter over `options`, returning `(original
/// index, label)` for each match.
fn fuzzy_filter<'a>(options: &[&'a str], query: &str) -> Vec<(usize, &'a str)> {
    let ql = query.to_lowercase();
    options
        .iter()
        .enumerate()
        .filter(|(_, o)| ql.is_empty() || o.to_lowercase().contains(&ql))
        .map(|(i, o)| (i, *o))
        .collect()
}

/// Full-screen fuzzy picker. Returns the index into the **original** `options`,
/// or `None` on cancel.
pub fn fuzzy_select(prompt: &str, options: &[&str]) -> Option<usize> {
    if prefers_inline() {
        return super::inline::inline_fuzzy_select(prompt, options);
    }
    if options.is_empty() || !is_interactive() {
        return None;
    }
    run(
        FuzzyState {
            query: vec![],
            cursor: 0,
        },
        |f, st| {
            let area = f.area();
            let z = layout::rows(area, [Length(1), Length(1), Min(1), Length(1)]);
            widgets::header(f, prompt, z[0]);

            let q: String = st.query.iter().collect();
            widgets::query_line(f, &q, z[1]);

            let filtered = fuzzy_filter(options, &q);
            let clamped = st.cursor.min(filtered.len().saturating_sub(1));
            let opts: Vec<SelectOption> = filtered
                .iter()
                .map(|(_, l)| SelectOption::new(*l))
                .collect();
            widgets::list(f, &opts, clamped, None, z[2]);

            widgets::help(
                f,
                &[
                    ("type", "filter"),
                    ("↑/↓", "move"),
                    ("Enter", "select"),
                    ("Esc", "cancel"),
                ],
                z[3],
            );
        },
        |st, key| match key {
            KeyCode::Down => {
                let n = fuzzy_filter(options, &st.query.iter().collect::<String>()).len();
                st.cursor = (st.cursor + 1).min(n.saturating_sub(1));
                Flow::Continue
            }
            KeyCode::Up => {
                st.cursor = st.cursor.saturating_sub(1);
                Flow::Continue
            }
            KeyCode::Backspace => {
                st.query.pop();
                st.cursor = 0;
                Flow::Continue
            }
            KeyCode::Char(c) => {
                st.query.push(c);
                st.cursor = 0;
                Flow::Continue
            }
            KeyCode::Enter => {
                let q: String = st.query.iter().collect();
                let filtered = fuzzy_filter(options, &q);
                let clamped = st.cursor.min(filtered.len().saturating_sub(1));
                Flow::Done(filtered.get(clamped).map(|(orig, _)| *orig))
            }
            KeyCode::Esc => Flow::Done(None),
            _ => Flow::Continue,
        },
    )
}
