//! Drawing functions for the diff TUI.
//!
//! All ratatui rendering logic lives here, separated from state and key handling
//! for clarity and potential reuse.

use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::commands::Ctx;
use crate::diff::highlight::HighlightedLine;
use crate::diff::parse::{FileDiff, Status};
use crate::diff::reviews;
use crate::ui::theme;

use super::state::{relative_time, AppState, DiffLayout, DisplayNote};

// ─── Top-level draw ─────────────────────────────────────────────────────────

/// Single-frame top-level drawer.  Layout: status -> body (sidebar + main) -> help.
pub fn draw(f: &mut ratatui::Frame, state: &mut AppState, _ctx: &Ctx<'_>) {
    let area = f.area();

    let [status_a, body_a, help_a] = ratatui::layout::Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_status_bar(f, state, status_a);

    let scrollbar_w = 2u16;
    let (main_a, scrollbar_a) = if state.show_sidebar {
        let sidebar_w = u16::min(25, body_a.width / 3);
        let [sidebar_a, main_a, scrollbar_a] = ratatui::layout::Layout::horizontal([
            Constraint::Length(sidebar_w),
            Constraint::Min(0),
            Constraint::Length(scrollbar_w),
        ])
        .areas(body_a);
        draw_sidebar(f, state, sidebar_a);
        (main_a, scrollbar_a)
    } else {
        let [main_a, scrollbar_a] = ratatui::layout::Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(scrollbar_w),
        ])
        .areas(body_a);
        (main_a, scrollbar_a)
    };

    draw_main(f, state, main_a);
    draw_scrollbar(f, state, scrollbar_a);

    draw_help_bar(f, state, help_a);

    if state.show_help {
        draw_help_overlay(f, area);
    }
    if state.editor.is_some() {
        draw_comment_editor(f, state, area);
    }

    state.last_viewport_h = main_a.height as usize;
}

// ─── Header ─────────────────────────────────────────────────────────────────

fn draw_status_bar(f: &mut ratatui::Frame, state: &AppState, area: Rect) {
    let t = theme::current();

    // Left side: file path + stats
    let title = state
        .current()
        .map(|(file, _)| format!("{} {}", status_glyph(file.status), file.path))
        .unwrap_or_else(|| "no files".to_string());
    let stat = state
        .current()
        .map(|(file, _)| file.stat())
        .unwrap_or_default();
    let stat_str = format!("+{} -{}", stat.added, stat.deleted);

    let left_spans: Vec<Span<'static>> = vec![
        Span::raw(" "),
        Span::styled(
            title,
            Style::default()
                .fg(t.palette.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(stat_str, Style::default().fg(t.palette.muted)),
    ];

    // Right side: file count
    let file_count = format!("{}/{} ", state.file_idx + 1, state.files.len());
    let right_span = Span::styled(
        file_count,
        Style::default()
            .fg(t.palette.text)
            .add_modifier(Modifier::BOLD),
    );

    // Render left-aligned and right-aligned parts
    let left_line = Line::from(left_spans);
    let right_line = Line::from(vec![right_span]);

    // Split area for left and right alignment
    let [left_area, right_area] = ratatui::layout::Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(right_line.width() as u16),
    ])
    .areas(area);

    f.render_widget(Paragraph::new(left_line), left_area);
    f.render_widget(Paragraph::new(right_line), right_area);
}

// ─── Sidebar ────────────────────────────────────────────────────────────────

fn draw_sidebar(f: &mut ratatui::Frame, state: &mut AppState, area: Rect) {
    use crate::ui::interactive::widgets::{file_tree, FileEntry};

    // Convert FileDiff entries to FileEntry for the tree widget
    let entries: Vec<FileEntry> = state
        .files
        .iter()
        .map(|(file, _)| {
            let stat = file.stat();
            FileEntry {
                path: file.path.clone(),
                status: status_char(file.status),
                added: stat.added,
                deleted: stat.deleted,
            }
        })
        .collect();

    // Calculate viewport height and total lines for scroll bounds
    let viewport_height = area.height.saturating_sub(2) as usize;
    let total_lines = count_tree_lines(&entries);

    // Clamp scroll to valid range
    let max_scroll = total_lines.saturating_sub(viewport_height);
    if state.sidebar_scroll > max_scroll {
        state.sidebar_scroll = max_scroll;
    }

    file_tree(
        f,
        &entries,
        state.file_idx,
        state.sidebar_scroll,
        area,
        true,
    );
}

/// Ensure the selected file is visible in the sidebar by adjusting scroll.
/// Call this when file selection changes (Tab, number keys, etc.)
pub fn ensure_sidebar_file_visible(state: &mut AppState, viewport_height: usize) {
    use crate::ui::interactive::widgets::FileEntry;

    let entries: Vec<FileEntry> = state
        .files
        .iter()
        .map(|(file, _)| {
            let stat = file.stat();
            FileEntry {
                path: file.path.clone(),
                status: status_char(file.status),
                added: stat.added,
                deleted: stat.deleted,
            }
        })
        .collect();

    let selected_row = find_visual_row_for_file(&entries, state.file_idx);

    if selected_row < state.sidebar_scroll {
        state.sidebar_scroll = selected_row;
    } else if selected_row >= state.sidebar_scroll + viewport_height {
        state.sidebar_scroll = selected_row.saturating_sub(viewport_height - 1);
    }
}

/// Find the visual row index for a file in the tree (accounting for directory headers).
fn find_visual_row_for_file(
    entries: &[crate::ui::interactive::widgets::FileEntry],
    file_idx: usize,
) -> usize {
    use std::collections::BTreeMap;

    // Group files by directory (same logic as build_tree_nodes)
    let mut dirs: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, file) in entries.iter().enumerate() {
        let dir = if let Some(pos) = file.path.rfind('/') {
            file.path[..pos].to_string()
        } else {
            String::new()
        };
        dirs.entry(dir).or_default().push(idx);
    }

    let mut row = 0;
    for (dir, file_indices) in dirs {
        if !dir.is_empty() {
            row += 1;
        }
        for idx in file_indices {
            if idx == file_idx {
                return row;
            }
            row += 1;
        }
    }
    row
}

/// Count total lines in the tree (files + directory headers).
fn count_tree_lines(entries: &[crate::ui::interactive::widgets::FileEntry]) -> usize {
    use std::collections::BTreeSet;

    let mut dirs: BTreeSet<String> = BTreeSet::new();
    for file in entries {
        if let Some(pos) = file.path.rfind('/') {
            dirs.insert(file.path[..pos].to_string());
        }
    }
    // Total = number of files + number of non-empty directories
    entries.len() + dirs.len()
}

/// Convert Status to a single character for display.
fn status_char(status: Status) -> char {
    match status {
        Status::Added => 'A',
        Status::Modified => 'M',
        Status::Deleted => 'D',
        Status::Renamed => 'R',
        Status::Copied => 'C',
        Status::ModeChange => 'T',
        Status::Binary => 'B',
        Status::TypeChange => 'T',
    }
}

// ─── Main content ───────────────────────────────────────────────────────────

fn draw_main(f: &mut ratatui::Frame, state: &AppState, area: Rect) {
    let Some((file, lines)) = state.current() else {
        return;
    };
    match state.layout {
        DiffLayout::Stack => draw_stack(f, state, file, lines, area),
        DiffLayout::Split => draw_split(f, state, file, lines, area),
        DiffLayout::Side => draw_side(f, state, file, lines, area),
    }
}

fn draw_stack(
    f: &mut ratatui::Frame,
    state: &AppState,
    _file: &FileDiff,
    lines: &[HighlightedLine],
    area: Rect,
) {
    let t = theme::current();
    let viewport = area.height as usize;
    let start = state.scroll.min(lines.len().saturating_sub(viewport));
    let end = (start + viewport).min(lines.len());

    let mut rendered: Vec<Line> = Vec::with_capacity(viewport * 2);
    #[allow(clippy::needless_range_loop)]
    for i in start..end {
        rendered.push(render_line(&lines[i], state, i));

        if let Some(note) = state.inline_notes.get(&(state.file_idx, i)) {
            let note_width = (area.width as usize).saturating_sub(20).max(30);
            for note_line in render_display_note(note, note_width) {
                let mut padded = vec![Span::raw("                ")];
                padded.extend(note_line.spans);
                rendered.push(Line::from(padded));
            }
        }
    }

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(t.palette.divider));
    f.render_widget(Paragraph::new(rendered).block(block), area);
}

fn draw_split(
    f: &mut ratatui::Frame,
    state: &AppState,
    file: &FileDiff,
    lines: &[HighlightedLine],
    area: Rect,
) {
    draw_stack(f, state, file, lines, area);
}

fn draw_side(
    f: &mut ratatui::Frame,
    state: &AppState,
    _file: &FileDiff,
    lines: &[HighlightedLine],
    area: Rect,
) {
    let viewport = area.height as usize;
    let t = theme::current();

    let [left_a, right_a] = ratatui::layout::Layout::horizontal([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .areas(area);

    let pairs: Vec<(Option<&HighlightedLine>, Option<&HighlightedLine>)> =
        align_side_by_side(lines);

    let total = pairs.len();
    let start = state.scroll.min(total.saturating_sub(viewport));
    let end = (start + viewport).min(total);

    let mut left_lines: Vec<Line> = Vec::new();
    let mut right_lines: Vec<Line> = Vec::new();

    #[allow(clippy::needless_range_loop)] // Index `i` is used for both pairs access and note lookup
    for i in start..end {
        left_lines.push(render_side(pairs[i].0, true, i, state));
        right_lines.push(render_side(pairs[i].1, false, i, state));

        if let Some(note) = state.inline_notes.get(&(state.file_idx, i)) {
            let note_width = (left_a.width as usize).saturating_sub(8).max(20);
            for note_line in render_display_note(note, note_width) {
                let mut padded = vec![Span::raw("  ")];
                padded.extend(note_line.spans);
                left_lines.push(Line::from(padded));
                right_lines.push(Line::raw(""));
            }
        }
    }

    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(t.palette.divider));
    f.render_widget(Paragraph::new(left_lines).block(block), left_a);
    f.render_widget(Paragraph::new(right_lines), right_a);
}

// ─── Scrollbar ──────────────────────────────────────────────────────────────

fn draw_scrollbar(f: &mut ratatui::Frame, state: &AppState, area: Rect) {
    let t = theme::current();
    let height = area.height as usize;
    if height == 0 {
        return;
    }

    let file_total = state.total_lines();
    let file_viewport = state.last_viewport_h.max(1);

    let mut total_lines = 0usize;
    let mut lines_before_current = 0usize;
    for (i, (_, lines)) in state.files.iter().enumerate() {
        if i < state.file_idx {
            lines_before_current += lines.len();
        }
        total_lines += lines.len();
    }
    let global_pos = lines_before_current + state.scroll;
    let global_viewport = file_viewport.min(file_total);

    let mut lines: Vec<Line> = Vec::with_capacity(height);

    for row in 0..height {
        let row_frac = row as f64 / height as f64;
        let row_frac_end = (row + 1) as f64 / height as f64;

        let file_char = if file_total == 0 {
            '░'
        } else {
            let scroll_start = state.scroll as f64 / file_total as f64;
            let scroll_end =
                (state.scroll + file_viewport).min(file_total) as f64 / file_total as f64;
            if row_frac_end > scroll_start && row_frac < scroll_end {
                '█'
            } else {
                '░'
            }
        };

        let global_char = if total_lines == 0 {
            '░'
        } else {
            let global_start = global_pos as f64 / total_lines as f64;
            let global_end = (global_pos + global_viewport) as f64 / total_lines as f64;
            if row_frac_end > global_start && row_frac < global_end {
                '█'
            } else {
                '░'
            }
        };

        lines.push(Line::from(vec![
            Span::styled(
                file_char.to_string(),
                Style::default().fg(t.palette.primary),
            ),
            Span::styled(
                global_char.to_string(),
                Style::default().fg(t.palette.accent),
            ),
        ]));
    }

    f.render_widget(Paragraph::new(lines), area);
}

// ─── Help bar ───────────────────────────────────────────────────────────────

fn draw_help_bar(f: &mut ratatui::Frame, state: &AppState, area: Rect) {
    let t = theme::current();

    // Left side: key hints
    let hints: &[(&str, &str)] = if state.show_help {
        &[("?", "close help"), ("q", "quit")]
    } else {
        &[
            ("j/k", "scroll"),
            ("Tab", "file"),
            ("n/p", "hunk"),
            ("s", "layout"),
            ("w", "wrap"),
            ("t", "tree"),
            ("?", "help"),
            ("q", "quit"),
        ]
    };
    let left_str: String = hints
        .iter()
        .flat_map(|(k, d)| [format!("{} {}", k, d), "  ".to_string()])
        .collect();

    // Right side: layout indicator + wrap status
    let mut right_parts: Vec<Span<'static>> = Vec::new();
    if state.wrap {
        right_parts.push(Span::styled("wrap", Style::default().fg(t.palette.success)));
        right_parts.push(Span::raw(" "));
    }
    right_parts.push(Span::styled(
        format!("{} ", state.layout.label()),
        Style::default().fg(t.palette.primary),
    ));

    let right_width: usize = right_parts.iter().map(|s| s.content.len()).sum();

    // Split area for left and right alignment
    let [left_area, right_area] = ratatui::layout::Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(right_width as u16),
    ])
    .areas(area);

    f.render_widget(
        Paragraph::new(Span::styled(left_str, Style::default().fg(t.palette.muted))),
        left_area,
    );
    f.render_widget(Paragraph::new(Line::from(right_parts)), right_area);
}

// ─── Help overlay ───────────────────────────────────────────────────────────

/// A section with a title and key bindings.
struct HelpSection {
    title: &'static str,
    bindings: &'static [(&'static str, &'static str)],
}

fn draw_help_overlay(f: &mut ratatui::Frame, area: Rect) {
    let t = theme::current();

    // Grouped sections for better organization
    let sections: &[HelpSection] = &[
        HelpSection {
            title: "Navigation",
            bindings: &[
                ("j  k", "cursor up/down"),
                ("g  G", "top / bottom"),
                ("PgDn PgUp", "page up/down"),
                ("n  p", "next/prev hunk"),
                ("Tab", "next/prev file"),
                ("1-9", "jump to file"),
            ],
        },
        HelpSection {
            title: "View",
            bindings: &[
                ("s", "cycle layout"),
                ("w", "toggle wrap"),
                ("t", "toggle tree"),
                ("[ ]", "scroll tree"),
                ("{ }", "scroll tree fast"),
            ],
        },
        HelpSection {
            title: "Notes",
            bindings: &[
                ("c", "add note"),
                ("e", "edit note"),
                ("d", "delete note"),
                ("J  K", "select lines"),
            ],
        },
        HelpSection {
            title: "",
            bindings: &[("?", "close help"), ("q", "quit")],
        },
    ];

    // Calculate dimensions
    let width = 56u16.min(area.width.saturating_sub(4));
    let total_lines: usize = sections
        .iter()
        .map(|s| {
            let title_lines = if s.title.is_empty() { 0 } else { 2 };
            title_lines + s.bindings.len()
        })
        .sum();
    let height = (total_lines as u16 + 5).min(area.height.saturating_sub(4));

    let overlay = crate::ui::interactive::widgets::popup_rect(area, width, height);
    f.render_widget(Clear, overlay);

    // Build content lines
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from("")); // top padding

    for (section_idx, section) in sections.iter().enumerate() {
        // Section title
        if !section.title.is_empty() {
            if section_idx > 0 {
                lines.push(Line::from("")); // spacing between sections
            }
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    section.title.to_uppercase(),
                    Style::default()
                        .fg(t.palette.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        // Bindings in this section
        for (key, desc) in section.bindings.iter() {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    format!("{:<12}", key),
                    Style::default()
                        .fg(t.palette.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(*desc, Style::default().fg(t.palette.text)),
            ]));
        }
    }

    // Styled border with rounded corners
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(t.palette.accent))
        .style(Style::default().bg(Color::Black))
        .title(Span::styled(
            " Keyboard Shortcuts ",
            Style::default()
                .fg(t.palette.accent)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(ratatui::layout::Alignment::Center);

    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(Color::Black)),
        overlay,
    );
}

// ─── Comment editor overlay ─────────────────────────────────────────────────

fn draw_comment_editor(f: &mut ratatui::Frame, state: &AppState, area: Rect) {
    let t = theme::current();
    let Some(editor) = state.editor.as_ref() else {
        return;
    };

    let width = 70u16.min(area.width.saturating_sub(4));
    let buffer_lines: Vec<&str> = editor.buffer.split('\n').collect();
    let line_count = buffer_lines.len().max(1);

    let height = (line_count as u16 + 3)
        .min(area.height.saturating_sub(4))
        .max(5);
    let v_center = area.height / 2;
    let h_center = (area.width - width) / 2;
    let popup = Rect {
        x: area.x + h_center,
        y: v_center.saturating_sub(height / 2),
        width,
        height,
    };

    f.render_widget(Clear, popup);

    let side_label = match editor.anchor.side {
        reviews::CommentSide::New => "+",
        reviews::CommentSide::Old => "-",
    };
    let border_color = t.palette.accent;
    let title = format!(
        " note · {}{}{}  ↵ save · Ctrl+↵ newline · esc cancel ",
        editor.anchor.path, side_label, editor.anchor.line,
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(Color::Black))
        .title(Span::styled(
            title,
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        ));

    let mut byte_offset = 0usize;
    let mut cursor_line_idx = 0usize;
    let mut cursor_col_byte = editor.cursor;

    for (i, line) in buffer_lines.iter().enumerate() {
        let line_end = byte_offset + line.len();
        if editor.cursor <= line_end {
            cursor_line_idx = i;
            cursor_col_byte = editor.cursor - byte_offset;
            break;
        }
        byte_offset = line_end + 1;
        cursor_line_idx = i + 1;
        cursor_col_byte = 0;
    }

    let mut lines: Vec<Line<'static>> = Vec::new();

    for (i, line_text) in buffer_lines.iter().enumerate() {
        let mut spans: Vec<Span<'static>> = Vec::new();

        if i == cursor_line_idx {
            let cursor_pos = cursor_col_byte.min(line_text.len());
            let (head, tail) = line_text.split_at(cursor_pos);

            spans.push(Span::styled(
                head.to_string(),
                Style::default().fg(t.palette.text),
            ));

            if tail.is_empty() {
                spans.push(Span::styled("█", Style::default().fg(t.palette.primary)));
            } else {
                let mut chars = tail.chars();
                let next_char = chars
                    .next()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| " ".to_string());
                let rest: String = chars.collect();
                spans.push(Span::styled(
                    next_char,
                    Style::default()
                        .bg(t.palette.primary)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(rest, Style::default().fg(t.palette.text)));
            }
        } else {
            spans.push(Span::styled(
                line_text.to_string(),
                Style::default().fg(t.palette.text),
            ));
        }

        lines.push(Line::from(spans));
    }

    if lines.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "█",
            Style::default().fg(t.palette.primary),
        )]));
    }

    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: false }),
        popup,
    );
}

// ─── Line rendering helpers ─────────────────────────────────────────────────

fn render_line(line: &HighlightedLine, state: &AppState, row: usize) -> Line<'static> {
    let t = theme::current();
    let marker_color = marker_color(line.marker);

    let is_cursor = row == state.cursor;
    let (sel_start, sel_end) = state.selection_range();
    let is_selected = row >= sel_start && row <= sel_end && state.selection_start.is_some();

    let line_bg = match line.marker {
        '+' => Some(Color::Rgb(0, 50, 0)),
        '-' => Some(Color::Rgb(50, 0, 0)),
        _ => None,
    };

    let cursor_bg = Color::Rgb(60, 60, 80);

    let row_bg = |s: Style| -> Style {
        if is_cursor {
            s.bg(cursor_bg)
        } else if let Some(bg) = line_bg {
            s.bg(bg)
        } else {
            s
        }
    };

    let with_bg = |s: Style| -> Style {
        if let Some(bg) = line_bg {
            s.bg(bg)
        } else {
            s
        }
    };

    let mut spans: Vec<Span<'static>> = Vec::with_capacity(line.spans.len() + 8);

    let cursor_style = Style::default()
        .fg(Color::Yellow)
        .bg(cursor_bg)
        .add_modifier(Modifier::BOLD);
    let selected_style = with_bg(Style::default().fg(t.palette.accent));
    let normal_style = with_bg(Style::default());

    if is_cursor {
        spans.push(Span::styled("▶ ", cursor_style));
    } else if is_selected {
        spans.push(Span::styled("│ ", selected_style));
    } else {
        spans.push(Span::styled("  ", normal_style));
    }

    let gutter_style = if is_cursor {
        Style::default()
            .fg(t.palette.primary)
            .bg(cursor_bg)
            .add_modifier(Modifier::BOLD)
    } else if is_selected {
        row_bg(Style::default().fg(t.palette.accent))
    } else {
        row_bg(Style::default().fg(t.palette.muted))
    };

    if let Some(n) = line.old_no {
        spans.push(Span::styled(format!("{:>4}", n), gutter_style));
    } else {
        spans.push(Span::styled("    ", row_bg(Style::default())));
    }
    spans.push(Span::styled(" ", row_bg(Style::default())));
    if let Some(n) = line.new_no {
        spans.push(Span::styled(format!("{:>4}", n), gutter_style));
    } else {
        spans.push(Span::styled("    ", row_bg(Style::default())));
    }
    spans.push(Span::styled(" ", row_bg(Style::default())));

    if state.annotated_rows.contains(&(state.file_idx, row)) {
        spans.push(Span::styled(
            "▌",
            row_bg(
                Style::default()
                    .fg(t.palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ));
    } else {
        spans.push(Span::styled(" ", row_bg(Style::default())));
    }
    spans.push(Span::styled(" ", row_bg(Style::default())));

    let marker_style = row_bg(
        Style::default()
            .fg(marker_color)
            .add_modifier(Modifier::BOLD),
    );
    spans.push(Span::styled(line.marker.to_string(), marker_style));
    spans.push(Span::styled(" ", row_bg(Style::default())));

    for (style, text) in &line.spans {
        spans.push(Span::styled(text.clone(), row_bg(*style)));
    }

    spans.push(Span::styled(" ".repeat(200), row_bg(Style::default())));

    Line::from(spans)
}

fn render_side(
    line: Option<&HighlightedLine>,
    is_left: bool,
    row: usize,
    state: &AppState,
) -> Line<'static> {
    let t = theme::current();
    let is_cursor = row == state.cursor;
    let cursor_bg = Color::Rgb(60, 60, 80);

    let Some(line) = line else {
        if is_cursor {
            return Line::from(vec![Span::styled(
                " ".repeat(100),
                Style::default().bg(cursor_bg),
            )]);
        }
        return Line::raw("");
    };

    if line.is_nonewline {
        let style = if is_cursor {
            Style::default()
                .fg(t.palette.muted)
                .bg(cursor_bg)
                .add_modifier(Modifier::ITALIC)
        } else {
            Style::default()
                .fg(t.palette.muted)
                .add_modifier(Modifier::ITALIC)
        };
        return Line::from(vec![Span::styled(" No newline at end of file", style)]);
    }

    let line_bg = match line.marker {
        '+' => Some(Color::Rgb(0, 50, 0)),
        '-' => Some(Color::Rgb(50, 0, 0)),
        _ => None,
    };
    let row_bg = |s: Style| -> Style {
        if is_cursor {
            s.bg(cursor_bg)
        } else if let Some(bg) = line_bg {
            s.bg(bg)
        } else {
            s
        }
    };

    let marker_color = marker_color(line.marker);
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(line.spans.len() + 6);

    let show_cursor_here = is_cursor && (is_left || line.marker == '+');
    if is_left || line.marker == '+' {
        if show_cursor_here {
            spans.push(Span::styled(
                "▶ ",
                Style::default()
                    .fg(Color::Yellow)
                    .bg(cursor_bg)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled("  ", row_bg(Style::default())));
        }
    }

    let no = if is_left {
        line.old_no.map(|n| format!("{:>4}", n))
    } else {
        line.new_no.map(|n| format!("{:>4}", n))
    };
    let gutter_style = if is_cursor {
        Style::default()
            .fg(t.palette.primary)
            .bg(cursor_bg)
            .add_modifier(Modifier::BOLD)
    } else {
        row_bg(Style::default().fg(t.palette.muted))
    };
    spans.push(Span::styled(
        no.unwrap_or_else(|| "    ".to_string()),
        gutter_style,
    ));
    spans.push(Span::styled(" ", row_bg(Style::default())));

    if (line.marker == '-' && is_left) || (line.marker == '+' && !is_left) {
        spans.push(Span::styled(
            line.marker.to_string(),
            row_bg(
                Style::default()
                    .fg(marker_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ));
    } else if line.marker == ' ' || line.marker == '\\' {
        spans.push(Span::styled(
            line.marker.to_string(),
            row_bg(Style::default().fg(marker_color)),
        ));
    } else {
        spans.push(Span::styled(" ", row_bg(Style::default())));
    }
    spans.push(Span::styled(" ", row_bg(Style::default())));

    for (style, text) in &line.spans {
        spans.push(Span::styled(text.clone(), row_bg(*style)));
    }

    spans.push(Span::styled(" ".repeat(200), row_bg(Style::default())));

    Line::from(spans)
}

// ─── Note rendering ─────────────────────────────────────────────────────────

fn render_display_note(note: &DisplayNote, max_width: usize) -> Vec<Line<'static>> {
    let note_fg = Color::Rgb(255, 235, 160);
    let border_fg = Color::Rgb(140, 130, 70);
    let label_fg = Color::Rgb(255, 220, 100);

    let content_width = max_width.saturating_sub(4).max(20);
    let wrapped_lines = wrap_text(&note.body, content_width);

    let time_str = note
        .created_at
        .map(relative_time)
        .unwrap_or_else(|| "just now".to_string());
    let base_meta = format!("{} · L{}", time_str, note.line);

    let content_max = wrapped_lines.iter().map(|l| l.len()).max().unwrap_or(10);
    let box_width = content_max.max(base_meta.len()).max(10).min(content_width);

    let author_str = match (&note.author_name, &note.author_email) {
        (Some(name), _) => {
            let avail = box_width.saturating_sub(base_meta.len() + 4);
            if avail >= 3 && name.len() > avail {
                format!(" — {}...", &name[..avail.saturating_sub(3)])
            } else if avail >= name.len() {
                format!(" — {}", name)
            } else {
                String::new()
            }
        }
        (None, Some(email)) => {
            let avail = box_width.saturating_sub(base_meta.len() + 4);
            if avail >= 3 && email.len() > avail {
                format!(" — {}...", &email[..avail.saturating_sub(3)])
            } else if avail >= email.len() {
                format!(" — {}", email)
            } else {
                String::new()
            }
        }
        (None, None) => String::new(),
    };
    let meta = format!("{}{}", base_meta, author_str);

    let mut result: Vec<Line<'static>> = Vec::new();

    let type_label = "Note";

    let label = format!(" {} ", type_label);
    let border_len = (box_width + 2).saturating_sub(label.len());
    let left_border = border_len / 2;
    let right_border = border_len - left_border;
    result.push(Line::from(vec![
        Span::styled("╭", Style::default().fg(border_fg)),
        Span::styled("─".repeat(left_border), Style::default().fg(border_fg)),
        Span::styled(
            label,
            Style::default().fg(label_fg).add_modifier(Modifier::BOLD),
        ),
        Span::styled("─".repeat(right_border), Style::default().fg(border_fg)),
        Span::styled("╮", Style::default().fg(border_fg)),
    ]));

    for line in &wrapped_lines {
        result.push(Line::from(vec![
            Span::styled("│", Style::default().fg(border_fg)),
            Span::styled(
                format!(" {:<width$} ", line, width = box_width),
                Style::default().fg(note_fg),
            ),
            Span::styled("│", Style::default().fg(border_fg)),
        ]));
    }

    result.push(Line::from(vec![
        Span::styled("│", Style::default().fg(border_fg)),
        Span::styled(
            format!(" {:<width$} ", meta, width = box_width),
            Style::default().fg(Color::Rgb(120, 120, 120)),
        ),
        Span::styled("│", Style::default().fg(border_fg)),
    ]));

    result.push(Line::from(vec![Span::styled(
        format!("╰{}╯", "─".repeat(box_width + 2)),
        Style::default().fg(border_fg),
    )]));

    result
}

// ─── Utility functions ──────────────────────────────────────────────────────

fn align_side_by_side(
    lines: &[HighlightedLine],
) -> Vec<(Option<&HighlightedLine>, Option<&HighlightedLine>)> {
    let mut out = Vec::with_capacity(lines.len());
    for l in lines {
        match l.marker {
            ' ' => out.push((Some(l), Some(l))),
            '+' => out.push((None, Some(l))),
            '-' => out.push((Some(l), None)),
            '\\' => out.push((Some(l), Some(l))),
            _ => out.push((Some(l), Some(l))),
        }
    }
    out
}

fn marker_color(marker: char) -> Color {
    let t = theme::current();
    match marker {
        '+' => t.palette.success,
        '-' => t.palette.danger,
        ' ' => t.palette.text,
        '\\' => t.palette.muted,
        _ => t.palette.text,
    }
}

fn status_glyph(s: Status) -> &'static str {
    match s {
        Status::Added => "+",
        Status::Modified => "~",
        Status::Deleted => "-",
        Status::Renamed => "→",
        Status::Copied => "⇉",
        Status::ModeChange => "#",
        Status::Binary => "□",
        Status::TypeChange => "↦",
    }
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();

    for paragraph in text.lines() {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        let words: Vec<&str> = paragraph.split_whitespace().collect();
        let mut current_line = String::new();

        for word in words {
            if current_line.is_empty() {
                if word.len() > max_width {
                    let mut remaining = word;
                    while remaining.len() > max_width {
                        lines.push(remaining[..max_width].to_string());
                        remaining = &remaining[max_width..];
                    }
                    current_line = remaining.to_string();
                } else {
                    current_line = word.to_string();
                }
            } else if current_line.len() + 1 + word.len() <= max_width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}
