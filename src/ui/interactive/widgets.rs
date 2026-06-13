//! Theme-aware building blocks for interactive screens.
//!
//! These are the reusable visual pieces — a slash header, a help bar, a cursor
//! list, an input line, a paginator — plus [`scroll_list`], which composes a
//! header + windowed list + paginator + help bar into a single call so list
//! prompts stay a few lines long.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph, Widget};
use ratatui_cheese::fieldset::{Fieldset, FieldsetFill, FieldsetStyles};
use ratatui_cheese::help::{Binding, Help, HelpStyles};
use ratatui_cheese::paginator::{Paginator, PaginatorMode, PaginatorState, PaginatorStyles};

use super::layout::four_zone;
use super::SelectOption;
use crate::ui::theme;

/// `(key, action)` hint pairs shown in a help bar.
pub type Hints<'a> = &'a [(&'a str, &'a str)];

/// Render a slash-fieldset header with the prompt title.
pub fn header(f: &mut ratatui::Frame, title: &str, area: Rect) {
    let t = theme::current();
    let padded = format!("  {}  ", title);
    Fieldset::new()
        .title(padded.as_str())
        .fill(FieldsetFill::Slash)
        .top_alignment(Alignment::Left)
        .styles(FieldsetStyles {
            title: Style::default()
                .fg(t.palette.muted)
                .add_modifier(Modifier::BOLD),
            rule: Style::default().fg(t.palette.divider),
        })
        .render(area, f.buffer_mut());
}

/// Render a help bar from `(key, action)` pairs.
pub fn help(f: &mut ratatui::Frame, items: Hints, area: Rect) {
    let t = theme::current();
    let key = Style::default().fg(t.palette.text);
    let desc = Style::default().fg(t.palette.muted);
    let styles = HelpStyles {
        ellipsis: desc,
        short_key: key,
        short_desc: desc,
        short_separator: desc,
        full_key: key,
        full_desc: desc,
        full_separator: desc,
    };
    let bindings: Vec<Binding> = items.iter().map(|(k, d)| Binding::new(*k, *d)).collect();
    Help::default()
        .styles(styles)
        .bindings(bindings)
        .render(area, f.buffer_mut());
}

/// Render a cursor list. `selected = Some(checks)` switches to checkbox mode.
pub fn list(
    f: &mut ratatui::Frame,
    options: &[SelectOption],
    cursor: usize,
    selected: Option<&[bool]>,
    area: Rect,
) {
    let t = theme::current();
    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let is_cur = i == cursor;
            let accent = if is_cur {
                t.palette.primary
            } else {
                t.palette.muted
            };

            let mut spans = vec![Span::styled(
                if is_cur { "  > " } else { "    " },
                Style::default().fg(accent),
            )];

            if let Some(checks) = selected {
                let checked = checks.get(i).copied().unwrap_or(false);
                spans.push(Span::styled(
                    if checked { "[✓] " } else { "[ ] " },
                    Style::default().fg(if checked {
                        t.palette.success
                    } else {
                        t.palette.muted
                    }),
                ));
            }

            spans.push(Span::styled(
                &opt.label,
                if is_cur {
                    Style::default()
                        .fg(t.palette.primary)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(t.palette.text)
                },
            ));

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

/// Render a paginator position indicator for a windowed list.
pub fn paginator(f: &mut ratatui::Frame, total: usize, per_page: usize, page: usize, area: Rect) {
    let t = theme::current();
    let mut state = PaginatorState::new(total, per_page);
    for _ in 0..page {
        state.next_page();
    }
    f.render_stateful_widget(
        Paginator::default()
            .mode(PaginatorMode::Arabic)
            .styles(PaginatorStyles {
                active: Style::default().fg(t.palette.primary),
                inactive: Style::default().fg(t.palette.muted),
            }),
        area,
        &mut state,
    );
}

/// Render a single-line text input with a block-reversed cursor cell.
pub fn input_line(f: &mut ratatui::Frame, before: &str, cursor_ch: &str, after: &str, area: Rect) {
    let t = theme::current();
    let line = Line::from(vec![
        Span::styled("  > ", Style::default().fg(t.palette.primary)),
        Span::raw(before),
        Span::styled(cursor_ch, Style::default().add_modifier(Modifier::REVERSED)),
        Span::raw(after),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

/// Render a fuzzy-search query line with a trailing block cursor.
pub fn query_line(f: &mut ratatui::Frame, query: &str, area: Rect) {
    let t = theme::current();
    let line = Line::from(vec![
        Span::styled("  > ", Style::default().fg(t.palette.primary)),
        Span::raw(query),
        Span::styled(
            "█",
            Style::default()
                .fg(t.palette.primary)
                .add_modifier(Modifier::REVERSED),
        ),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

/// Render a `Yes / No` toggle, highlighting the active choice.
pub fn yes_no(f: &mut ratatui::Frame, yes_selected: bool, area: Rect) {
    let t = theme::current();
    let on = Modifier::BOLD | Modifier::REVERSED;
    let yes = if yes_selected {
        Style::default().fg(t.palette.success).add_modifier(on)
    } else {
        Style::default().fg(t.palette.muted)
    };
    let no = if yes_selected {
        Style::default().fg(t.palette.muted)
    } else {
        Style::default().fg(t.palette.danger).add_modifier(on)
    };
    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled("  Yes  ", yes),
        Span::raw("   "),
        Span::styled("  No  ", no),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

/// Render an error message line (used by validated inputs).
pub fn error_line(f: &mut ratatui::Frame, msg: &str, area: Rect) {
    let t = theme::current();
    f.render_widget(
        Paragraph::new(Span::styled(
            format!("  ✗  {}", msg),
            Style::default().fg(t.palette.danger),
        )),
        area,
    );
}

/// Compose a complete scrolling list screen into `area`:
/// header + the visible window of `options` + a paginator (when overflowing) +
/// a help bar. This is the whole body of `select` / `multi_select`.
pub fn scroll_list(
    f: &mut ratatui::Frame,
    area: Rect,
    title: &str,
    options: &[SelectOption],
    cursor: usize,
    selected: Option<&[bool]>,
    hints: Hints,
) {
    let n = options.len();
    let [header_a, body_a, pager_a, help_a] = four_zone(area);
    let content_h = body_a.height as usize;
    let per_page = content_h.max(1);
    let page = cursor / per_page;
    let start = page * per_page;
    let end = (start + per_page).min(n);
    let local = cursor - start;

    header(f, title, header_a);
    list(
        f,
        &options[start..end],
        local,
        selected.map(|s| &s[start..end]),
        body_a,
    );
    if n > content_h && content_h > 0 {
        paginator(f, n, per_page, page, pager_a);
    }
    help(f, hints, help_a);
}
