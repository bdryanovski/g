//! Mode 4 — inline (non-fullscreen) interactive prompts.
//!
//! This folder is the **reusable inline kit**. Prompts render into the normal
//! terminal scroll buffer (no alternate screen) and stay in history. The shared
//! machinery lives in:
//!
//! - [`runtime`] — the TTY guard, the static [`runtime::header`], and the
//!   raw-mode key loop [`runtime::run_raw`].
//! - [`widgets`] — option/checkbox row rendering and the in-place
//!   [`widgets::redraw_rows`].
//!
//! Each prompt below prints its header + initial body, then hands the key loop
//! to `run_raw` — so the navigation logic reads top-to-bottom with no
//! enable/disable-raw-mode bookkeeping.

mod runtime;
mod widgets;

use std::io::{self, Write};

use crossterm::cursor;
use crossterm::event::KeyCode;
use crossterm::execute;
use crossterm::terminal::{self, ClearType};

use super::interactive::SelectOption;
use super::print::{muted, paint_text, primary, success, warning};
use super::render::indent;
use runtime::{is_interactive, run_raw, Flow};

// ─── inline_select ────────────────────────────────────────────────────────────

/// Inline single-choice list with `j/k`/arrow navigation and scrollable viewport.
/// Returns the chosen index, or `None` on cancel.
pub fn inline_select(prompt: &str, options: &[SelectOption]) -> Option<usize> {
    let n = options.len();
    if n == 0 || !is_interactive() {
        return None;
    }

    let mut stdout = io::stdout();
    let max_label = widgets::max_label_width(options);
    let mut view = widgets::ScrollView::new(n);
    let mut prev_lines = 0usize;

    // Print header with item count for long lists
    let header_text = if n > view.visible {
        format!("{} ({} items)", prompt, n)
    } else {
        prompt.to_string()
    };
    runtime::header(&header_text, "j/k ↑↓  move   Enter  select   q  cancel");

    // Initial render
    prev_lines = render_select_view(&view, options, max_label, prev_lines, &mut stdout);

    run_raw(|key| match key {
        KeyCode::Char('j') | KeyCode::Down => {
            view.move_down();
            prev_lines = render_select_view(&view, options, max_label, prev_lines, &mut stdout);
            Flow::Continue
        }
        KeyCode::Char('k') | KeyCode::Up => {
            view.move_up();
            prev_lines = render_select_view(&view, options, max_label, prev_lines, &mut stdout);
            Flow::Continue
        }
        KeyCode::Enter => {
            let _ = write!(stdout, "\r\n");
            let _ = stdout.flush();
            Flow::Done(Some(view.cursor))
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            let _ = write!(stdout, "\r\n");
            let _ = stdout.flush();
            Flow::Done(None)
        }
        _ => Flow::Continue,
    })
    .flatten()
}

/// Render the single-select view with scrolling support.
fn render_select_view(
    view: &widgets::ScrollView,
    options: &[SelectOption],
    max_label: usize,
    prev_lines: usize,
    stdout: &mut io::Stdout,
) -> usize {
    widgets::redraw_scrollable(view, prev_lines, stdout, |i, is_cursor, w| {
        widgets::option_row(&options[i], is_cursor, max_label, w);
    })
}

// ─── inline_multi_select ─────────────────────────────────────────────────────

/// Inline checkbox list with scrollable viewport. `pre_selected` marks items
/// that start checked. Returns the indices of checked items (empty on cancel).
///
/// For long lists, only a portion of items is visible at once, with scroll
/// indicators showing how many items are above/below the viewport.
pub fn inline_multi_select(
    prompt: &str,
    options: &[SelectOption],
    pre_selected: &[bool],
) -> Vec<usize> {
    let n = options.len();
    if n == 0 || !is_interactive() {
        return vec![];
    }

    let mut stdout = io::stdout();
    let max_label = widgets::max_label_width(options);
    let mut checked: Vec<bool> = (0..n)
        .map(|i| pre_selected.get(i).copied().unwrap_or(false))
        .collect();
    let mut view = widgets::ScrollView::new(n);
    let mut prev_lines = 0usize;

    // Print header with item count
    let header_text = if n > view.visible {
        format!("{} ({} items)", prompt, n)
    } else {
        prompt.to_string()
    };
    runtime::header(
        &header_text,
        "j/k ↑↓  move   Space  toggle   a  all   n  none   Enter  confirm   q  cancel",
    );

    // Initial render
    prev_lines = render_multi_view(&view, options, &checked, max_label, prev_lines, &mut stdout);

    let result = run_raw(|key| {
        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                view.move_down();
                prev_lines =
                    render_multi_view(&view, options, &checked, max_label, prev_lines, &mut stdout);
                Flow::Continue
            }
            KeyCode::Char('k') | KeyCode::Up => {
                view.move_up();
                prev_lines =
                    render_multi_view(&view, options, &checked, max_label, prev_lines, &mut stdout);
                Flow::Continue
            }
            KeyCode::Char(' ') => {
                checked[view.cursor] = !checked[view.cursor];
                prev_lines =
                    render_multi_view(&view, options, &checked, max_label, prev_lines, &mut stdout);
                Flow::Continue
            }
            KeyCode::Char('a') => {
                let all = checked.iter().all(|&c| c);
                checked.iter_mut().for_each(|c| *c = !all);
                prev_lines =
                    render_multi_view(&view, options, &checked, max_label, prev_lines, &mut stdout);
                Flow::Continue
            }
            KeyCode::Char('n') => {
                checked.iter_mut().for_each(|c| *c = false);
                prev_lines =
                    render_multi_view(&view, options, &checked, max_label, prev_lines, &mut stdout);
                Flow::Continue
            }
            KeyCode::Enter => {
                let _ = write!(stdout, "\r\n");
                let _ = stdout.flush();
                Flow::Done(
                    checked
                        .iter()
                        .enumerate()
                        .filter_map(|(i, &c)| c.then_some(i))
                        .collect::<Vec<_>>(),
                )
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                let _ = write!(stdout, "\r\n");
                let _ = stdout.flush();
                Flow::Done(vec![])
            }
            _ => Flow::Continue,
        }
    });

    result.unwrap_or_default()
}

/// Render the multi-select view with scrolling support.
fn render_multi_view(
    view: &widgets::ScrollView,
    options: &[SelectOption],
    checked: &[bool],
    max_label: usize,
    prev_lines: usize,
    stdout: &mut io::Stdout,
) -> usize {
    widgets::redraw_scrollable(view, prev_lines, stdout, |i, is_cursor, w| {
        widgets::multi_row(&options[i], is_cursor, checked[i], max_label, w);
    })
}

// ─── inline_fuzzy_select ─────────────────────────────────────────────────────

/// Inline list picker for string slices. Returns the index into `options`.
pub fn inline_fuzzy_select(prompt: &str, options: &[&str]) -> Option<usize> {
    let owned: Vec<SelectOption> = options.iter().map(|&s| SelectOption::new(s)).collect();
    inline_select(prompt, &owned)
}

// ─── inline_input_validated ───────────────────────────────────────────────────

/// Inline single-line text input with live echo and validation. On `Enter` the
/// `validate` closure runs; an `Err(msg)` prints the error and re-prompts.
/// `Esc` cancels and returns `None`.
pub fn inline_input_validated<F>(prompt: &str, default: Option<&str>, validate: F) -> Option<String>
where
    F: Fn(&str) -> Result<(), String>,
{
    if !is_interactive() {
        return default.map(str::to_owned);
    }

    loop {
        // Static prompt (cooked mode), optionally showing the default.
        print!("{}{}  {}  ", indent(), primary("›"), paint_text(prompt));
        if let Some(d) = default {
            if !d.is_empty() {
                print!("{} ", muted(&format!("[{d}]")));
            }
        }
        io::stdout().flush().ok();

        let mut chars: Vec<char> = Vec::new();
        let mut cursor: usize = 0;

        let line: Option<String> = run_raw(|key| match key {
            KeyCode::Char(c) => {
                chars.insert(cursor, c);
                cursor += 1;
                let tail: String = chars[cursor..].iter().collect();
                print!("{c}{tail}");
                if !tail.is_empty() {
                    let _ = execute!(io::stdout(), cursor::MoveLeft(tail.len() as u16));
                }
                io::stdout().flush().ok();
                Flow::Continue
            }
            KeyCode::Backspace => {
                if cursor > 0 {
                    cursor -= 1;
                    chars.remove(cursor);
                    let tail: String = chars[cursor..].iter().collect();
                    let _ = execute!(io::stdout(), cursor::MoveLeft(1));
                    print!("{tail}");
                    let _ = execute!(io::stdout(), terminal::Clear(ClearType::UntilNewLine));
                    if !tail.is_empty() {
                        let _ = execute!(io::stdout(), cursor::MoveLeft(tail.len() as u16));
                    }
                    io::stdout().flush().ok();
                }
                Flow::Continue
            }
            KeyCode::Delete => {
                if cursor < chars.len() {
                    chars.remove(cursor);
                    let tail: String = chars[cursor..].iter().collect();
                    print!("{tail}");
                    let _ = execute!(io::stdout(), terminal::Clear(ClearType::UntilNewLine));
                    if !tail.is_empty() {
                        let _ = execute!(io::stdout(), cursor::MoveLeft(tail.len() as u16));
                    }
                    io::stdout().flush().ok();
                }
                Flow::Continue
            }
            KeyCode::Left => {
                if cursor > 0 {
                    cursor -= 1;
                    let _ = execute!(io::stdout(), cursor::MoveLeft(1));
                    io::stdout().flush().ok();
                }
                Flow::Continue
            }
            KeyCode::Right => {
                if cursor < chars.len() {
                    cursor += 1;
                    let _ = execute!(io::stdout(), cursor::MoveRight(1));
                    io::stdout().flush().ok();
                }
                Flow::Continue
            }
            KeyCode::Home => {
                if cursor > 0 {
                    let _ = execute!(io::stdout(), cursor::MoveLeft(cursor as u16));
                    cursor = 0;
                    io::stdout().flush().ok();
                }
                Flow::Continue
            }
            KeyCode::End => {
                if cursor < chars.len() {
                    let forward = chars.len() - cursor;
                    let _ = execute!(io::stdout(), cursor::MoveRight(forward as u16));
                    cursor = chars.len();
                    io::stdout().flush().ok();
                }
                Flow::Continue
            }
            KeyCode::Enter => {
                print!("\r\n");
                io::stdout().flush().ok();
                let s: String = chars.iter().collect();
                let s = if s.is_empty() {
                    default.unwrap_or("").to_string()
                } else {
                    s
                };
                Flow::Done(Some(s))
            }
            KeyCode::Esc => {
                print!("\r\n");
                io::stdout().flush().ok();
                Flow::Done(None)
            }
            _ => Flow::Continue,
        })
        .flatten();

        match line {
            None => return None,
            Some(s) => match validate(&s) {
                Ok(()) => return Some(s),
                Err(msg) => println!("{}{}  {}", indent(), warning("✗"), muted(&msg)),
            },
        }
    }
}

/// Inline text input without validation. Convenience wrapper.
#[allow(dead_code)]
pub fn inline_input(prompt: &str, default: Option<&str>) -> Option<String> {
    inline_input_validated(prompt, default, |_| Ok(()))
}

// ─── inline_confirm ───────────────────────────────────────────────────────────

/// Inline yes/no prompt reading a single keypress. `Enter`/`Esc` accept the
/// `default`. Returns `default` when non-interactive.
pub fn inline_confirm(prompt: &str, default: bool) -> bool {
    if !is_interactive() {
        return default;
    }

    let hint = if default {
        format!("{}/{}", success("Y"), muted("n"))
    } else {
        format!("{}/{}", muted("y"), success("N"))
    };
    print!(
        "{}{}  {}  {}{}{}  ",
        indent(),
        primary("›"),
        paint_text(prompt),
        muted("["),
        hint,
        muted("]"),
    );
    io::stdout().flush().ok();

    run_raw(|key| match key {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            print!("y\r\n");
            io::stdout().flush().ok();
            Flow::Done(true)
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            print!("n\r\n");
            io::stdout().flush().ok();
            Flow::Done(false)
        }
        KeyCode::Enter | KeyCode::Esc => {
            print!("{}\r\n", if default { "y" } else { "n" });
            io::stdout().flush().ok();
            Flow::Done(default)
        }
        _ => Flow::Continue,
    })
    .unwrap_or(default)
}
