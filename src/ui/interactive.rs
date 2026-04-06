//! Mode 3 — full-screen ratatui TUI interactive components.
//! Every public function in this module:
//! 1. Calls [`ratatui::init()`] to enter full-screen alternate-screen mode.
//! 2. Runs an event loop that redraws on every keystroke.
//! 3. Calls [`ratatui::restore()`] before returning the result.
//!
//! All screens render a [`ratatui_cheese::help::Help`] bar at the bottom
//! with context-sensitive keybinding hints, and a slash fieldset header
//! from [`ratatui_cheese::fieldset::Fieldset`] at the top.
//!
//! # Non-TTY safety
//!
//! All entry points check [`std::io::IsTerminal`] before entering TUI mode.
//! If stdin is not a terminal (piped input, CI), they return the `default`
//! value immediately so scripts never block.

use std::io::IsTerminal;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui_cheese::paginator::{Paginator, PaginatorMode, PaginatorState, PaginatorStyles};

use super::render::{is_inline_prompts, is_no_interactive};

/// Return `true` when interactive TUI prompts are allowed.
///
/// Returns `false` when:
/// - `--no-interactive` was passed on the command line, or
/// - stdin is not a TTY (piped input, CI environment).
#[inline]
fn is_interactive() -> bool {
    !is_no_interactive() && std::io::stdin().is_terminal()
}
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph, Widget};
use ratatui_cheese::fieldset::{Fieldset, FieldsetFill, FieldsetStyles};
use ratatui_cheese::help::{Binding, Help, HelpStyles};

use super::theme;

// ─── Public data types ────────────────────────────────────────────────────────

/// A selectable item with an optional short description rendered to the right.
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

// ─── Internal: layout helpers ─────────────────────────────────────────────────

/// Build custom [`HelpStyles`] from the active theme.
#[allow(dead_code)]
fn help_styles() -> HelpStyles {
    let t = theme::current();
    let key_style = Style::default().fg(t.palette.text);
    let desc_style = Style::default().fg(t.palette.muted);
    HelpStyles {
        ellipsis: desc_style,
        short_key: key_style,
        short_desc: desc_style,
        short_separator: desc_style,
        full_key: key_style,
        full_desc: desc_style,
        full_separator: desc_style,
    }
}

/// Render a ratatui-cheese slash fieldset header inside a [`ratatui::Frame`].
#[allow(dead_code)]
fn render_header(f: &mut ratatui::Frame, title: &str, area: Rect) {
    let t = theme::current();
    let rule_style = Style::default().fg(t.palette.divider);
    let title_style = Style::default()
        .fg(t.palette.muted)
        .add_modifier(Modifier::BOLD);
    let padded = format!("  {}  ", title);
    Fieldset::new()
        .title(padded.as_str())
        .fill(FieldsetFill::Slash)
        .top_alignment(Alignment::Left)
        .styles(FieldsetStyles {
            title: title_style,
            rule: rule_style,
        })
        .render(area, f.buffer_mut());
}

/// Render a ratatui-cheese Help bar with the provided `(key, action)` pairs.
#[allow(dead_code)]
fn render_help(f: &mut ratatui::Frame, items: &[(&str, &str)], area: Rect) {
    let bindings: Vec<Binding> = items.iter().map(|(k, d)| Binding::new(*k, *d)).collect();
    Help::default()
        .styles(help_styles())
        .bindings(bindings)
        .render(area, f.buffer_mut());
}

/// Standard 3-zone layout: `[header=1][content=fill][help=1]`.
#[allow(dead_code)]
fn three_zone(area: Rect) -> [Rect; 3] {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);
    [chunks[0], chunks[1], chunks[2]]
}

/// Four-zone layout adding a paginator row: `[header=1][content=fill][pager=1][help=1]`.
fn four_zone(area: Rect) -> [Rect; 4] {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    [chunks[0], chunks[1], chunks[2], chunks[3]]
}

/// Build [`PaginatorStyles`] from the active theme palette.
fn paginator_styles() -> PaginatorStyles {
    let t = theme::current();
    PaginatorStyles {
        active: Style::default().fg(t.palette.primary),
        inactive: Style::default().fg(t.palette.muted),
    }
}

/// Render a list cursor indicator and items into `area`.
#[allow(dead_code)]
fn render_list(
    f: &mut ratatui::Frame,
    options: &[SelectOption],
    cursor: usize,
    selected: Option<&[bool]>, // `None` = single-select, `Some` = checkboxes
    area: Rect,
) {
    let t = theme::current();
    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let is_cur = i == cursor;
            let is_checked = selected.map(|s| s[i]).unwrap_or(false);

            let prefix = if is_cur { "  > " } else { "    " };
            let prefix_style = Style::default().fg(if is_cur {
                t.palette.primary
            } else {
                t.palette.muted
            });

            let mut spans: Vec<Span> = vec![Span::styled(prefix, prefix_style)];

            // Checkbox (multi-select mode)
            if selected.is_some() {
                let checkbox = if is_checked { "[✓] " } else { "[ ] " };
                let cb_style = if is_checked {
                    Style::default().fg(t.palette.success)
                } else {
                    Style::default().fg(t.palette.muted)
                };
                spans.push(Span::styled(checkbox, cb_style));
            }

            // Label
            let label_style = if is_cur {
                Style::default()
                    .fg(t.palette.primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.palette.text)
            };
            spans.push(Span::styled(&opt.label, label_style));

            // Description
            if let Some(desc) = &opt.description {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    desc.as_str(),
                    Style::default().fg(t.palette.muted),
                ));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    f.render_widget(List::new(items), area);
}

// ─── select ───────────────────────────────────────────────────────────────────

/// Present a full-screen list picker and return the selected index.
///
/// When the list is longer than the visible area, scrolls automatically and
/// shows a ratatui-cheese [`Paginator`] position indicator.
///
/// Returns `None` when the user presses `q` or `Esc` to cancel.
/// Falls back to `None` immediately if stdin is not a TTY or `--no-interactive`.
pub fn select(prompt: &str, options: &[SelectOption]) -> Option<usize> {
    if is_inline_prompts() {
        return super::inline::inline_select(prompt, options);
    }
    let n = options.len();
    if n == 0 || !is_interactive() {
        return None;
    }

    let mut terminal = ratatui::init();
    let mut cursor = 0usize;

    let result = loop {
        let _ = terminal.draw(|f| {
            let area = f.area();
            // Content height = total height - header(1) - paginator(1) - help(1)
            let content_h = area.height.saturating_sub(3) as usize;
            let needs_pager = n > content_h && content_h > 0;

            let [header, content, pager, help] = four_zone(area);

            render_header(f, prompt, header);

            // Build items for visible window only (manual scroll offset).
            let per_page = content_h.max(1);
            let page = cursor / per_page;
            let (start, end) = {
                let s = page * per_page;
                let e = (s + per_page).min(n);
                (s, e)
            };
            let visible = &options[start..end];
            let local_cursor = cursor - start;

            render_list(f, visible, local_cursor, None, content);

            // Paginator (arabic "current/total items")
            if needs_pager {
                let mut pag_state = PaginatorState::new(n, per_page);
                for _ in 0..page {
                    pag_state.next_page();
                }
                f.render_stateful_widget(
                    Paginator::default()
                        .mode(PaginatorMode::Arabic)
                        .styles(paginator_styles()),
                    pager,
                    &mut pag_state,
                );
            }

            render_help(
                f,
                &[("j/k", "move"), ("Enter", "select"), ("q", "quit")],
                help,
            );
        });

        match event::read() {
            Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => match k.code {
                KeyCode::Char('j') | KeyCode::Down => cursor = (cursor + 1).min(n - 1),
                KeyCode::Char('k') | KeyCode::Up => cursor = cursor.saturating_sub(1),
                KeyCode::Enter => break Some(cursor),
                KeyCode::Esc | KeyCode::Char('q') => break None,
                _ => {}
            },
            _ => {}
        }
    };

    ratatui::restore();
    result
}

// ─── multi_select ─────────────────────────────────────────────────────────────

/// Present a full-screen checkbox list and return the indices of checked items.
///
/// When the list is longer than the visible area, scrolls automatically and
/// shows a ratatui-cheese [`Paginator`] position indicator.
///
/// Returns an empty `Vec` when the user cancels (`Esc` / `q`).
/// Falls back to an empty `Vec` if stdin is not a TTY or `--no-interactive`.
pub fn multi_select(prompt: &str, options: &[SelectOption]) -> Vec<usize> {
    if is_inline_prompts() {
        return super::inline::inline_multi_select(prompt, options, &[]);
    }
    let n = options.len();
    if n == 0 || !is_interactive() {
        return vec![];
    }

    let mut terminal = ratatui::init();
    let mut cursor = 0usize;
    let mut checked = vec![false; n];

    let result = loop {
        let _ = terminal.draw(|f| {
            let area = f.area();
            let content_h = area.height.saturating_sub(3) as usize;
            let needs_pager = n > content_h && content_h > 0;
            let [header, content, pager, help] = four_zone(area);

            render_header(f, prompt, header);

            let per_page = content_h.max(1);
            let page = cursor / per_page;
            let (start, end) = {
                let s = page * per_page;
                (s, (s + per_page).min(n))
            };
            let visible = &options[start..end];
            let visible_checked = &checked[start..end];
            let local_cursor = cursor - start;

            render_list(f, visible, local_cursor, Some(visible_checked), content);

            if needs_pager {
                let mut pag_state = PaginatorState::new(n, per_page);
                for _ in 0..page {
                    pag_state.next_page();
                }
                f.render_stateful_widget(
                    Paginator::default()
                        .mode(PaginatorMode::Arabic)
                        .styles(paginator_styles()),
                    pager,
                    &mut pag_state,
                );
            }

            render_help(
                f,
                &[
                    ("Space", "toggle"),
                    ("a", "all/none"),
                    ("Enter", "confirm"),
                    ("q", "cancel"),
                ],
                help,
            );
        });

        match event::read() {
            Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => match k.code {
                KeyCode::Char('j') | KeyCode::Down => cursor = (cursor + 1).min(n - 1),
                KeyCode::Char('k') | KeyCode::Up => cursor = cursor.saturating_sub(1),
                KeyCode::Char(' ') => checked[cursor] = !checked[cursor],
                KeyCode::Char('a') => {
                    let all = checked.iter().all(|&s| s);
                    checked.iter_mut().for_each(|s| *s = !all);
                }
                KeyCode::Enter => {
                    break checked
                        .iter()
                        .enumerate()
                        .filter_map(|(i, &s)| if s { Some(i) } else { None })
                        .collect();
                }
                KeyCode::Esc | KeyCode::Char('q') => break vec![],
                _ => {}
            },
            _ => {}
        }
    };

    ratatui::restore();
    result
}

// ─── input ────────────────────────────────────────────────────────────────────

/// Present a full-screen single-line text input.
///
/// Returns `None` when the user presses `Esc` to cancel.
/// Falls back to `default.map(str::to_owned)` if stdin is not a TTY.
pub fn input(prompt: &str, default: Option<&str>) -> Option<String> {
    input_validated(prompt, default, |_| Ok(()))
}

/// Text input with a validation function called on `Enter`.
///
/// `validate` receives the current string and returns `Ok(())` to accept or
/// `Err(message)` to keep the input open and display the error.
///
/// Returns `None` on `Esc`/cancel.
/// Falls back to `default.map(str::to_owned)` if stdin is not a TTY.
pub fn input_validated<F>(prompt: &str, default: Option<&str>, validate: F) -> Option<String>
where
    F: Fn(&str) -> Result<(), String>,
{
    if is_inline_prompts() {
        return super::inline::inline_input_validated(prompt, default, validate);
    }
    if !is_interactive() {
        return default.map(str::to_owned);
    }

    let mut terminal = ratatui::init();
    let mut chars: Vec<char> = default.unwrap_or("").chars().collect();
    let mut cursor = chars.len();
    let mut error: Option<String> = None;

    let result = loop {
        let t = theme::current();
        let before: String = chars[..cursor].iter().collect();
        let cursor_ch: String = chars
            .get(cursor)
            .map(|c| c.to_string())
            .unwrap_or_else(|| " ".to_string());
        let after: String = if cursor < chars.len() {
            chars[cursor + 1..].iter().collect()
        } else {
            String::new()
        };
        let err_clone = error.clone();

        let _ = terminal.draw(|f| {
            let area = f.area();
            // [header 1][blank 1][input 1][error/hint 1][filler][help 1]
            let chunks = Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(area);

            render_header(f, prompt, chunks[0]);

            // Input line
            let input_line = Line::from(vec![
                Span::styled("  > ", Style::default().fg(t.palette.primary)),
                Span::raw(&before),
                Span::styled(
                    &cursor_ch,
                    Style::default().add_modifier(Modifier::REVERSED),
                ),
                Span::raw(&after),
            ]);
            f.render_widget(Paragraph::new(input_line), chunks[2]);

            // Error message (if any)
            if let Some(ref msg) = err_clone {
                f.render_widget(
                    Paragraph::new(Span::styled(
                        format!("  ✗  {}", msg),
                        Style::default().fg(t.palette.danger),
                    )),
                    chunks[3],
                );
            }

            render_help(f, &[("Enter", "confirm"), ("Esc", "cancel")], chunks[5]);
        });

        match event::read() {
            Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => {
                error = None;
                match k.code {
                    KeyCode::Char(c) => {
                        chars.insert(cursor, c);
                        cursor += 1;
                    }
                    KeyCode::Backspace => {
                        if cursor > 0 {
                            chars.remove(cursor - 1);
                            cursor -= 1;
                        }
                    }
                    KeyCode::Delete => {
                        if cursor < chars.len() {
                            chars.remove(cursor);
                        }
                    }
                    KeyCode::Left => cursor = cursor.saturating_sub(1),
                    KeyCode::Right => cursor = (cursor + 1).min(chars.len()),
                    KeyCode::Home => cursor = 0,
                    KeyCode::End => cursor = chars.len(),
                    KeyCode::Enter => {
                        let value: String = chars.iter().collect();
                        match validate(&value) {
                            Ok(()) => break Some(value),
                            Err(msg) => error = Some(msg),
                        }
                    }
                    KeyCode::Esc => break None,
                    _ => {}
                }
            }
            _ => {}
        }
    };

    ratatui::restore();
    result
}

// ─── confirm ─────────────────────────────────────────────────────────────────

/// Present a full-screen yes/no prompt and return the answer.
///
/// If stdin is not a TTY, returns `default` immediately.
pub fn confirm(prompt: &str, default: bool) -> bool {
    if is_inline_prompts() {
        return super::inline::inline_confirm(prompt, default);
    }
    if !is_interactive() {
        return default;
    }

    let mut terminal = ratatui::init();
    let mut choice = default;

    let result = loop {
        let t = theme::current();
        let c = choice; // copy for the closure

        let _ = terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(area);

            render_header(f, prompt, chunks[0]);

            let yes_style = if c {
                Style::default()
                    .fg(t.palette.success)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(t.palette.muted)
            };
            let no_style = if !c {
                Style::default()
                    .fg(t.palette.danger)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(t.palette.muted)
            };

            let line = Line::from(vec![
                Span::raw("  "),
                Span::styled("  Yes  ", yes_style),
                Span::raw("   "),
                Span::styled("  No  ", no_style),
            ]);
            f.render_widget(Paragraph::new(line), chunks[2]);

            render_help(
                f,
                &[
                    ("y/n", "choose"),
                    ("←/→", "toggle"),
                    ("Enter", "confirm"),
                    ("Esc", "cancel"),
                ],
                chunks[4],
            );
        });

        match event::read() {
            Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => match k.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => break true,
                KeyCode::Char('n') | KeyCode::Char('N') => break false,
                KeyCode::Left | KeyCode::Char('h') => choice = true,
                KeyCode::Right | KeyCode::Char('l') => choice = false,
                KeyCode::Enter => break choice,
                KeyCode::Esc => break default,
                _ => {}
            },
            _ => {}
        }
    };

    ratatui::restore();
    result
}

// ─── fuzzy_select ─────────────────────────────────────────────────────────────

/// Present a full-screen fuzzy search picker and return the index in the
/// **original** `options` slice of the selected item.
///
/// As the user types, the list is filtered by case-insensitive substring
/// matching.  Returns `None` on cancel.
/// Falls back to `None` if stdin is not a TTY.
pub fn fuzzy_select(prompt: &str, options: &[&str]) -> Option<usize> {
    if is_inline_prompts() {
        return super::inline::inline_fuzzy_select(prompt, options);
    }
    if options.is_empty() || !is_interactive() {
        return None;
    }

    let mut terminal = ratatui::init();
    let mut query: Vec<char> = vec![];
    let mut list_cursor: usize = 0;

    let result = loop {
        let query_str: String = query.iter().collect();
        let filtered: Vec<(usize, &&str)> = options
            .iter()
            .enumerate()
            .filter(|(_, opt)| {
                query_str.is_empty() || opt.to_lowercase().contains(&query_str.to_lowercase())
            })
            .collect();
        let n = filtered.len();
        let clamped = list_cursor.min(n.saturating_sub(1));
        let q = query_str.clone();
        let filt = filtered
            .iter()
            .enumerate()
            .map(|(fi, (_, label))| (fi, **label))
            .collect::<Vec<_>>();

        let _ = terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::vertical([
                Constraint::Length(1), // header
                Constraint::Length(1), // query input
                Constraint::Min(1),    // filtered list
                Constraint::Length(1), // help
            ])
            .split(area);

            let t = theme::current();
            render_header(f, prompt, chunks[0]);

            // Query input line
            let query_line = Line::from(vec![
                Span::styled("  > ", Style::default().fg(t.palette.primary)),
                Span::raw(&q),
                Span::styled(
                    "█",
                    Style::default()
                        .fg(t.palette.primary)
                        .add_modifier(Modifier::REVERSED),
                ),
            ]);
            f.render_widget(Paragraph::new(query_line), chunks[1]);

            // Filtered list
            let items: Vec<ListItem> = filt
                .iter()
                .map(|(fi, label)| {
                    let is_cur = *fi == clamped;
                    let prefix = if is_cur { "  > " } else { "    " };
                    let label_style = if is_cur {
                        Style::default()
                            .fg(t.palette.primary)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(t.palette.text)
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            prefix,
                            Style::default().fg(if is_cur {
                                t.palette.primary
                            } else {
                                t.palette.muted
                            }),
                        ),
                        Span::styled(*label, label_style),
                    ]))
                })
                .collect();
            f.render_widget(List::new(items), chunks[2]);

            render_help(
                f,
                &[
                    ("type", "filter"),
                    ("j/k", "move"),
                    ("Enter", "select"),
                    ("Esc", "cancel"),
                ],
                chunks[3],
            );
        });

        match event::read() {
            Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => match k.code {
                // Any printable character narrows the query; navigation uses arrows.
                KeyCode::Down => {
                    list_cursor = (clamped + 1).min(n.saturating_sub(1));
                }
                KeyCode::Up => {
                    list_cursor = clamped.saturating_sub(1);
                }
                KeyCode::Backspace => {
                    query.pop();
                    list_cursor = 0;
                }
                KeyCode::Char(c) => {
                    query.push(c);
                    list_cursor = 0;
                }
                KeyCode::Enter => {
                    break filtered.get(clamped).map(|(orig, _)| *orig);
                }
                KeyCode::Esc => break None,
                _ => {}
            },
            _ => {}
        }
    };

    ratatui::restore();
    result
}
