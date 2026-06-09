//! Mode 2 — inline (non-fullscreen) interactive prompts.
//!
//! All functions here render directly into the **normal terminal scroll buffer**
//! — no alternate screen, no `ratatui::init()`.  Output stays in history.
//!
//! ## How navigation works in [`inline_select`]
//!
//! 1. The option list is printed once into the scroll buffer (before raw mode).
//! 2. Raw mode is entered for keyboard input only.
//! 3. On cursor movement, crossterm moves the terminal cursor back to the first
//!    option line (`MoveToPreviousLine(n)`) and reprints all `n` lines in place.
//!    The rest of the scroll buffer above is never touched.
//! 4. On `Enter` / `Esc`, raw mode is restored and a one-line confirmation echo
//!    is printed.
//!
//! ## Why `\r\n` inside raw mode
//!
//! In raw mode, `\n` moves the cursor down but does **not** return it to
//! column 0.  Every newline inside a raw-mode print therefore uses `\r\n`.

use std::io::{self, IsTerminal, Write};

use crossterm::cursor;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{self, ClearType};
use crossterm::{execute, queue};

use super::interactive::SelectOption;
use super::print::{muted, paint_text, primary, success, warning};
use super::render::{indent, is_no_interactive};
use super::widgets::print_fieldset;

// ─── Guard ────────────────────────────────────────────────────────────────────

fn is_interactive() -> bool {
    !is_no_interactive() && io::stdin().is_terminal()
}

// ─── Internal rendering helpers ──────────────────────────────────────────────

/// Print one option row.
///
/// Uses `\r\n` so the function is safe to call both before and after
/// `enable_raw_mode()`.  In cooked mode `\r` is harmless.
fn print_option_row(
    _i: usize,
    opt: &SelectOption,
    is_cursor: bool,
    max_label: usize,
    stdout: &mut impl Write,
) {
    let cursor_ch = if is_cursor { primary(">") } else { muted(" ") };
    let label_style = if is_cursor {
        primary(&opt.label)
    } else {
        paint_text(&opt.label)
    };
    let pad = " ".repeat(max_label - opt.label.len());

    let line = match &opt.description {
        Some(desc) if !desc.is_empty() => format!(
            "{}{}  {}{}  {}\r\n",
            indent(),
            cursor_ch,
            label_style,
            pad,
            muted(desc),
        ),
        _ => format!("{}{}  {}\r\n", indent(), cursor_ch, label_style),
    };

    let _ = write!(stdout, "{line}");
}

/// Reprint the full option list in-place (used after cursor movement).
///
/// Moves the terminal cursor up exactly `n` lines (back to the first option),
/// then redraws every row, leaving the cursor one line below the last option —
/// the same position it was in before the call.
fn redraw_options(
    options: &[SelectOption],
    cursor: usize,
    max_label: usize,
    stdout: &mut impl Write,
) {
    let n = options.len();
    let _ = execute!(stdout, cursor::MoveToPreviousLine(n as u16));
    for (i, opt) in options.iter().enumerate() {
        let _ = queue!(stdout, terminal::Clear(ClearType::CurrentLine));
        print_option_row(i, opt, i == cursor, max_label, stdout);
    }
    let _ = stdout.flush();
}

// ─── inline_select ────────────────────────────────────────────────────────────

/// List picker with arrow-key / hjkl navigation that renders **inline**.
///
/// Prints the option list into the normal scroll buffer, then enters raw mode
/// to handle key events.  The selected row is highlighted with `>` and primary
/// color; navigation updates only the option lines in place.
///
/// Returns the 0-based index of the confirmed selection, or `None` on cancel.
pub fn inline_select(prompt: &str, options: &[SelectOption]) -> Option<usize> {
    let n = options.len();
    if n == 0 || !is_interactive() {
        return None;
    }

    let mut stdout = io::stdout();
    let max_label = options.iter().map(|o| o.label.len()).max().unwrap_or(0);
    let mut cursor = 0usize;

    // ── Static header (stays in scrollback, never overwritten) ────────────────
    println!();
    print_fieldset(prompt);
    println!();
    println!(
        "{}{}",
        indent(),
        muted("j/k ↑↓  move   Enter  select   q  cancel"),
    );
    println!();

    // ── Initial option list ───────────────────────────────────────────────────
    // Printed before raw mode so `println!` cooked-mode newlines work.
    for (i, opt) in options.iter().enumerate() {
        print_option_row(i, opt, i == cursor, max_label, &mut stdout);
    }
    stdout.flush().ok();

    // ── Raw mode: keyboard navigation ─────────────────────────────────────────
    terminal::enable_raw_mode().ok()?;

    let result = loop {
        match event::read() {
            Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => match k.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if cursor < n - 1 {
                        cursor += 1;
                        redraw_options(options, cursor, max_label, &mut stdout);
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if cursor > 0 {
                        cursor -= 1;
                        redraw_options(options, cursor, max_label, &mut stdout);
                    }
                }
                KeyCode::Enter => {
                    let _ = write!(stdout, "\r\n");
                    stdout.flush().ok();
                    break Some(cursor);
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    let _ = write!(stdout, "\r\n");
                    stdout.flush().ok();
                    break None;
                }
                _ => {}
            },
            _ => {}
        }
    };

    terminal::disable_raw_mode().ok();
    result
}

// ─── inline_multi_select ─────────────────────────────────────────────────────

/// Checkbox list with arrow-key navigation that renders **inline**.
///
/// `pre_selected` marks items that start checked (pass an empty slice or a
/// slice of `false` for none pre-selected).  Pressing `Space` toggles the
/// item at the cursor.  `a` toggles all; `n` clears all.
///
/// Returns the 0-based indices of all checked items.  Returns an empty `Vec`
/// on cancel.
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
    let max_label = options.iter().map(|o| o.label.len()).max().unwrap_or(0);
    let mut cursor = 0usize;
    let mut checked: Vec<bool> = (0..n)
        .map(|i| pre_selected.get(i).copied().unwrap_or(false))
        .collect();

    // ── Static header ─────────────────────────────────────────────────────────
    println!();
    print_fieldset(prompt);
    println!();
    println!(
        "{}{}",
        indent(),
        muted("j/k ↑↓  move   Space  toggle   a  all   n  none   Enter  confirm   q  cancel"),
    );
    println!();

    // ── Initial list ──────────────────────────────────────────────────────────
    for (i, opt) in options.iter().enumerate() {
        print_multi_row(i, opt, i == cursor, checked[i], max_label, &mut stdout);
    }
    stdout.flush().ok();

    // ── Raw mode ──────────────────────────────────────────────────────────────
    if terminal::enable_raw_mode().is_err() {
        return vec![];
    }

    let result = loop {
        match event::read() {
            Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => match k.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if cursor < n - 1 {
                        cursor += 1;
                        redraw_multi(options, cursor, &checked, max_label, &mut stdout);
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if cursor > 0 {
                        cursor -= 1;
                        redraw_multi(options, cursor, &checked, max_label, &mut stdout);
                    }
                }
                KeyCode::Char(' ') => {
                    checked[cursor] = !checked[cursor];
                    redraw_multi(options, cursor, &checked, max_label, &mut stdout);
                }
                KeyCode::Char('a') => {
                    let all = checked.iter().all(|&c| c);
                    checked.iter_mut().for_each(|c| *c = !all);
                    redraw_multi(options, cursor, &checked, max_label, &mut stdout);
                }
                KeyCode::Char('n') => {
                    checked.iter_mut().for_each(|c| *c = false);
                    redraw_multi(options, cursor, &checked, max_label, &mut stdout);
                }
                KeyCode::Enter => {
                    let _ = write!(stdout, "\r\n");
                    stdout.flush().ok();
                    break checked
                        .iter()
                        .enumerate()
                        .filter_map(|(i, &c)| if c { Some(i) } else { None })
                        .collect::<Vec<_>>();
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    let _ = write!(stdout, "\r\n");
                    stdout.flush().ok();
                    break vec![];
                }
                _ => {}
            },
            _ => {}
        }
    };

    terminal::disable_raw_mode().ok();
    result
}

fn print_multi_row(
    _idx: usize,
    opt: &SelectOption,
    is_cursor: bool,
    is_checked: bool,
    max_label: usize,
    stdout: &mut impl Write,
) {
    let cursor_ch = if is_cursor { primary(">") } else { muted(" ") };
    let checkbox = if is_checked {
        success("[✓]")
    } else {
        muted("[ ]")
    };
    let label_style = if is_cursor {
        primary(&opt.label)
    } else {
        paint_text(&opt.label)
    };
    let pad = " ".repeat(max_label - opt.label.len());
    let desc = opt.description.as_deref().unwrap_or("");

    let line = if desc.is_empty() {
        format!(
            "{}{}  {}  {}{}\r\n",
            indent(), cursor_ch, checkbox, label_style, pad,
        )
    } else {
        format!(
            "{}{}  {}  {}{}  {}\r\n",
            indent(),
            cursor_ch,
            checkbox,
            label_style,
            pad,
            muted(desc),
        )
    };
    let _ = write!(stdout, "{line}");
}

fn redraw_multi(
    options: &[SelectOption],
    cursor: usize,
    checked: &[bool],
    max_label: usize,
    stdout: &mut impl Write,
) {
    let n = options.len();
    let _ = execute!(stdout, cursor::MoveToPreviousLine(n as u16));
    for (i, opt) in options.iter().enumerate() {
        let _ = queue!(stdout, terminal::Clear(ClearType::CurrentLine));
        print_multi_row(i, opt, i == cursor, checked[i], max_label, stdout);
    }
    let _ = stdout.flush();
}

// ─── inline_fuzzy_select ─────────────────────────────────────────────────────

/// Numbered list picker (simplified version of [`inline_select`]) for string slices.
///
/// Displays all options as a navigable list and returns the index in the
/// **original** `options` slice, or `None` on cancel.
pub fn inline_fuzzy_select(prompt: &str, options: &[&str]) -> Option<usize> {
    let owned: Vec<SelectOption> = options.iter().map(|&s| SelectOption::new(s)).collect();
    inline_select(prompt, &owned)
}

// ─── inline_input_validated ───────────────────────────────────────────────────

/// Inline single-line text input with live echo and validation.
///
/// Reads one character at a time in raw mode, echoing as the user types.
/// On `Enter` the `validate` closure is called; an `Err(msg)` prints the error
/// and re-shows the prompt.  `Esc` cancels and returns `None`.
pub fn inline_input_validated<F>(prompt: &str, default: Option<&str>, validate: F) -> Option<String>
where
    F: Fn(&str) -> Result<(), String>,
{
    if !is_interactive() {
        return default.map(str::to_owned);
    }

    loop {
        // Print the prompt, optionally showing the default in brackets.
        print!("{}{}  {}  ", indent(), primary("›"), paint_text(prompt));
        if let Some(d) = default {
            if !d.is_empty() {
                print!("{} ", muted(&format!("[{d}]")));
            }
        }
        io::stdout().flush().ok();

        terminal::enable_raw_mode().ok();
        let mut chars: Vec<char> = Vec::new();
        let mut cursor: usize = 0;

        let line_result: Option<String> = loop {
            match event::read() {
                Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => match k.code {
                    KeyCode::Char(c) => {
                        chars.insert(cursor, c);
                        cursor += 1;
                        // Print the new char plus any tail that got shifted right,
                        // then move the terminal cursor back to the logical position.
                        let tail: String = chars[cursor..].iter().collect();
                        print!("{c}{tail}");
                        if !tail.is_empty() {
                            let _ = execute!(io::stdout(), cursor::MoveLeft(tail.len() as u16));
                        }
                        io::stdout().flush().ok();
                    }
                    KeyCode::Backspace => {
                        if cursor > 0 {
                            cursor -= 1;
                            chars.remove(cursor);
                            // Move terminal cursor left, reprint shifted tail,
                            // clear the leftover character at the end, then
                            // reposition the terminal cursor.
                            let tail: String = chars[cursor..].iter().collect();
                            let _ = execute!(io::stdout(), cursor::MoveLeft(1));
                            print!("{tail}");
                            let _ =
                                execute!(io::stdout(), terminal::Clear(ClearType::UntilNewLine));
                            if !tail.is_empty() {
                                let _ = execute!(io::stdout(), cursor::MoveLeft(tail.len() as u16));
                            }
                            io::stdout().flush().ok();
                        }
                    }
                    KeyCode::Delete => {
                        if cursor < chars.len() {
                            chars.remove(cursor);
                            // Reprint the shifted tail from the current cursor
                            // position, clear the leftover char at the end, then
                            // reposition.
                            let tail: String = chars[cursor..].iter().collect();
                            print!("{tail}");
                            let _ =
                                execute!(io::stdout(), terminal::Clear(ClearType::UntilNewLine));
                            if !tail.is_empty() {
                                let _ = execute!(io::stdout(), cursor::MoveLeft(tail.len() as u16));
                            }
                            io::stdout().flush().ok();
                        }
                    }
                    KeyCode::Left => {
                        if cursor > 0 {
                            cursor -= 1;
                            let _ = execute!(io::stdout(), cursor::MoveLeft(1));
                            io::stdout().flush().ok();
                        }
                    }
                    KeyCode::Right => {
                        if cursor < chars.len() {
                            cursor += 1;
                            let _ = execute!(io::stdout(), cursor::MoveRight(1));
                            io::stdout().flush().ok();
                        }
                    }
                    KeyCode::Home => {
                        if cursor > 0 {
                            let _ = execute!(io::stdout(), cursor::MoveLeft(cursor as u16));
                            cursor = 0;
                            io::stdout().flush().ok();
                        }
                    }
                    KeyCode::End => {
                        if cursor < chars.len() {
                            let forward = chars.len() - cursor;
                            let _ = execute!(io::stdout(), cursor::MoveRight(forward as u16));
                            cursor = chars.len();
                            io::stdout().flush().ok();
                        }
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
                        break Some(s);
                    }
                    KeyCode::Esc => {
                        print!("\r\n");
                        io::stdout().flush().ok();
                        break None;
                    }
                    _ => {}
                },
                _ => {}
            }
        };

        terminal::disable_raw_mode().ok();

        match line_result {
            None => return None,
            Some(s) => match validate(&s) {
                Ok(()) => return Some(s),
                Err(msg) => println!("{}{}  {}", indent(), warning("✗"), muted(&msg)),
            },
        }
    }
}

/// Inline text input without validation.  Convenience wrapper.
#[allow(dead_code)]
pub fn inline_input(prompt: &str, default: Option<&str>) -> Option<String> {
    inline_input_validated(prompt, default, |_| Ok(()))
}

// ─── inline_confirm ───────────────────────────────────────────────────────────

/// Inline yes/no prompt that reads a single keypress.
///
/// `Enter` and `Esc` both accept the `default`.
/// Returns `default` immediately when not in an interactive environment.
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

    terminal::enable_raw_mode().ok();

    let result = loop {
        match event::read() {
            Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => match k.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    print!("y\r\n");
                    io::stdout().flush().ok();
                    break true;
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    print!("n\r\n");
                    io::stdout().flush().ok();
                    break false;
                }
                KeyCode::Enter | KeyCode::Esc => {
                    let echo = if default { "y" } else { "n" };
                    print!("{echo}\r\n");
                    io::stdout().flush().ok();
                    break default;
                }
                _ => {}
            },
            _ => {}
        }
    };

    terminal::disable_raw_mode().ok();
    result
}
