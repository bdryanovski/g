//! Theme-aware building blocks for interactive screens.
//!
//! These are the reusable visual pieces — a slash header, a help bar, a cursor
//! list, an input line, a paginator — plus [`scroll_list`], which composes a
//! header + windowed list + paginator + help bar into a single call so list
//! prompts stay a few lines long.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Widget};
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

// ─── Popup widgets ──────────────────────────────────────────────────────────
//
// These widgets are available for use by any TUI screen that needs modal
// dialogs. Currently used by `diff_tui` for quit confirmation and comment
// editing.

/// Options for customizing popup appearance.
#[allow(dead_code)]
#[derive(Default)]
pub struct PopupStyle {
    /// Border color (defaults to theme accent).
    pub border_color: Option<Color>,
    /// Title color (defaults to border color).
    pub title_color: Option<Color>,
    /// Background color (defaults to black).
    pub bg_color: Option<Color>,
}


/// Calculate centered popup rect within `area`.
#[allow(dead_code)]
pub fn popup_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(4));
    let height = height.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect { x, y, width, height }
}

/// Render a confirmation popup with Yes/No options.
///
/// # Arguments
/// * `title` - Popup title shown in the border
/// * `message` - Main message text (can be multi-line)
/// * `yes_selected` - Whether "Yes" is currently highlighted
/// * `yes_label` - Custom label for Yes option (defaults to "Yes")
/// * `no_label` - Custom label for No option (defaults to "No")
/// * `style` - Optional popup styling
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)] // Widget API - parameters are all meaningful
pub fn popup_confirm(
    f: &mut ratatui::Frame,
    title: &str,
    message: &str,
    yes_selected: bool,
    yes_label: Option<&str>,
    no_label: Option<&str>,
    style: Option<PopupStyle>,
    area: Rect,
) {
    let t = theme::current();
    let style = style.unwrap_or_default();
    let border_color = style.border_color.unwrap_or(t.palette.accent);
    let title_color = style.title_color.unwrap_or(border_color);
    let bg_color = style.bg_color.unwrap_or(Color::Black);

    let yes_label = yes_label.unwrap_or("Yes");
    let no_label = no_label.unwrap_or("No");

    // Calculate dimensions based on content
    let message_lines: Vec<&str> = message.lines().collect();
    let max_line_len = message_lines.iter().map(|l| l.len()).max().unwrap_or(20);
    let width = (max_line_len as u16 + 6).max(30).min(area.width.saturating_sub(4));
    let height = (message_lines.len() as u16 + 6).min(area.height.saturating_sub(2));

    let popup = popup_rect(area, width, height);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(bg_color))
        .title(Span::styled(
            format!(" {} ", title),
            Style::default().fg(title_color).add_modifier(Modifier::BOLD),
        ));

    // Build content
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));
    for msg_line in message_lines {
        lines.push(Line::from(Span::styled(
            format!(" {} ", msg_line),
            Style::default().fg(t.palette.text),
        )));
    }
    lines.push(Line::from(""));

    // Yes/No buttons
    let on = Modifier::BOLD | Modifier::REVERSED;
    let yes_style = if yes_selected {
        Style::default().fg(t.palette.success).add_modifier(on)
    } else {
        Style::default().fg(t.palette.muted)
    };
    let no_style = if yes_selected {
        Style::default().fg(t.palette.muted)
    } else {
        Style::default().fg(t.palette.danger).add_modifier(on)
    };
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(format!(" [Y] {} ", yes_label), yes_style),
        Span::raw("   "),
        Span::styled(format!(" [N] {} ", no_label), no_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "  [Esc] Cancel",
            Style::default().fg(t.palette.muted),
        ),
    ]));

    f.render_widget(Paragraph::new(lines).block(block), popup);
}

/// Render a text input popup for collecting user input.
///
/// # Arguments
/// * `title` - Popup title shown in the border
/// * `prompt` - Instruction text above the input
/// * `buffer` - Current input buffer
/// * `cursor_pos` - Byte position of cursor in buffer
/// * `style` - Optional popup styling
/// * `multiline` - If true, shows instructions for multiline input
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)] // Widget API - parameters are all meaningful
pub fn popup_input(
    f: &mut ratatui::Frame,
    title: &str,
    prompt: &str,
    buffer: &str,
    cursor_pos: usize,
    style: Option<PopupStyle>,
    multiline: bool,
    area: Rect,
) {
    let t = theme::current();
    let style = style.unwrap_or_default();
    let border_color = style.border_color.unwrap_or(t.palette.accent);
    let title_color = style.title_color.unwrap_or(border_color);
    let bg_color = style.bg_color.unwrap_or(Color::Black);

    // Calculate dimensions
    let buffer_lines: Vec<&str> = buffer.split('\n').collect();
    let line_count = buffer_lines.len().max(1);
    let width = 60u16.min(area.width.saturating_sub(4));
    let height = (line_count as u16 + 5).min(area.height.saturating_sub(2)).max(6);

    let popup = popup_rect(area, width, height);
    f.render_widget(Clear, popup);

    let help_text = if multiline {
        "↵ save · Ctrl+↵ newline · Esc cancel"
    } else {
        "↵ save · Esc cancel"
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(bg_color))
        .title(Span::styled(
            format!(" {} · {} ", title, help_text),
            Style::default().fg(title_color).add_modifier(Modifier::BOLD),
        ));

    // Build content with cursor
    let mut lines: Vec<Line> = Vec::new();

    if !prompt.is_empty() {
        lines.push(Line::from(Span::styled(
            format!(" {} ", prompt),
            Style::default().fg(t.palette.muted),
        )));
    }

    // Find cursor line and position
    let mut byte_offset = 0usize;
    let mut cursor_line_idx = 0usize;
    let mut cursor_col_byte = cursor_pos;

    for (i, line) in buffer_lines.iter().enumerate() {
        let line_end = byte_offset + line.len();
        if cursor_pos <= line_end {
            cursor_line_idx = i;
            cursor_col_byte = cursor_pos - byte_offset;
            break;
        }
        byte_offset = line_end + 1; // +1 for newline
        cursor_line_idx = i + 1;
        cursor_col_byte = 0;
    }

    // Render buffer lines with cursor
    for (i, line_text) in buffer_lines.iter().enumerate() {
        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push(Span::raw(" "));

        if i == cursor_line_idx {
            let cursor_pos_in_line = cursor_col_byte.min(line_text.len());
            let (head, tail) = line_text.split_at(cursor_pos_in_line);

            spans.push(Span::styled(head.to_string(), Style::default().fg(t.palette.text)));

            if tail.is_empty() {
                spans.push(Span::styled("█", Style::default().fg(t.palette.primary)));
            } else {
                let mut chars = tail.chars();
                let next_char = chars.next().map(|c| c.to_string()).unwrap_or_else(|| " ".to_string());
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
            spans.push(Span::styled(line_text.to_string(), Style::default().fg(t.palette.text)));
        }

        lines.push(Line::from(spans));
    }

    // Handle empty buffer
    if lines.is_empty() || (lines.len() == 1 && prompt.is_empty()) {
        lines.push(Line::from(vec![
            Span::raw(" "),
            Span::styled("█", Style::default().fg(t.palette.primary)),
        ]));
    }

    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: false }),
        popup,
    );
}

// ─── File Tree Widget ───────────────────────────────────────────────────────
//
// A tree-style file list grouped by directory, with status indicators and stats.

/// A file entry for the tree widget.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Full path (e.g., "src/commands/git/diff.rs").
    pub path: String,
    /// Single-char status indicator (M, A, D, R, ?, etc.).
    pub status: char,
    /// Lines added.
    pub added: u32,
    /// Lines deleted.
    pub deleted: u32,
}

/// Internal node for building the tree.
#[derive(Debug)]
enum TreeNode {
    /// A directory header.
    Dir { name: String },
    /// A file entry.
    File {
        name: String,
        status: char,
        added: u32,
        deleted: u32,
        index: usize, // Original index for selection
    },
}

/// Build a flat list of tree nodes from file entries, grouped by directory.
fn build_tree_nodes(files: &[FileEntry]) -> Vec<TreeNode> {
    use std::collections::BTreeMap;

    // Group files by their parent directory
    let mut dirs: BTreeMap<String, Vec<(usize, &FileEntry)>> = BTreeMap::new();

    for (idx, file) in files.iter().enumerate() {
        let dir = if let Some(pos) = file.path.rfind('/') {
            file.path[..pos].to_string()
        } else {
            String::new() // Root level
        };
        dirs.entry(dir).or_default().push((idx, file));
    }

    let mut nodes = Vec::new();

    for (dir, dir_files) in dirs {
        // Add directory header if not root
        if !dir.is_empty() {
            nodes.push(TreeNode::Dir {
                name: format!("{}/", dir),
            });
        }

        // Add files under this directory
        for (idx, file) in dir_files {
            let name = if let Some(pos) = file.path.rfind('/') {
                file.path[pos + 1..].to_string()
            } else {
                file.path.clone()
            };
            nodes.push(TreeNode::File {
                name,
                status: file.status,
                added: file.added,
                deleted: file.deleted,
                index: idx,
            });
        }
    }

    nodes
}

/// Render a file tree with directory grouping.
///
/// # Arguments
/// * `files` - List of file entries to display
/// * `selected` - Currently selected file index (into the original `files` slice)
/// * `scroll` - Scroll offset (number of lines to skip from top)
/// * `area` - Area to render into
/// * `show_stats` - Whether to show +N -N stats
///
/// Returns the total number of lines in the tree (for scroll bounds).
#[allow(dead_code)]
pub fn file_tree(
    f: &mut ratatui::Frame,
    files: &[FileEntry],
    selected: usize,
    scroll: usize,
    area: Rect,
    show_stats: bool,
) -> usize {
    let t = theme::current();
    let nodes = build_tree_nodes(files);

    // Available width inside the block (minus border)
    let inner_width = area.width.saturating_sub(2) as usize;

    let items: Vec<ListItem> = nodes
        .iter()
        .map(|node| {
            match node {
                TreeNode::Dir { name } => {
                    // Directory header - muted color, no indentation
                    ListItem::new(Line::from(vec![Span::styled(
                        name.clone(),
                        Style::default()
                            .fg(t.palette.muted)
                            .add_modifier(Modifier::BOLD),
                    )]))
                }
                TreeNode::File {
                    name,
                    status,
                    added,
                    deleted,
                    index,
                } => {
                    let is_selected = *index == selected;

                    // Status color
                    let status_color = match status {
                        'M' | '~' => t.palette.warning,
                        'A' | '+' => t.palette.success,
                        'D' | '-' => t.palette.danger,
                        'R' => t.palette.accent,
                        '?' => t.palette.muted,
                        _ => t.palette.text,
                    };

                    // Build stats string first to calculate padding
                    let stats_str = if show_stats && (*added > 0 || *deleted > 0) {
                        let mut s = String::new();
                        if *added > 0 {
                            s.push_str(&format!("+{}", added));
                        }
                        if *deleted > 0 {
                            if !s.is_empty() {
                                s.push(' ');
                            }
                            s.push_str(&format!("-{}", deleted));
                        }
                        s
                    } else {
                        String::new()
                    };

                    // Left part: "M filename"
                    let left_part = format!("{} {}", status, name);
                    let left_len = left_part.len();
                    let stats_len = stats_str.len();

                    // Calculate padding to push stats to the right
                    let padding = if stats_len > 0 {
                        inner_width.saturating_sub(left_len + stats_len + 1)
                    } else {
                        0
                    };

                    let mut spans = vec![
                        Span::styled(
                            format!("{} ", status),
                            Style::default().fg(status_color),
                        ),
                        Span::styled(
                            name.clone(),
                            if is_selected {
                                Style::default()
                                    .fg(t.palette.primary)
                                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                            } else {
                                Style::default().fg(t.palette.text)
                            },
                        ),
                    ];

                    // Add padding and stats
                    if !stats_str.is_empty() {
                        spans.push(Span::raw(" ".repeat(padding)));
                        // Color the stats
                        if *added > 0 {
                            spans.push(Span::styled(
                                format!("+{}", added),
                                Style::default().fg(t.palette.success),
                            ));
                        }
                        if *deleted > 0 {
                            if *added > 0 {
                                spans.push(Span::raw(" "));
                            }
                            spans.push(Span::styled(
                                format!("-{}", deleted),
                                Style::default().fg(t.palette.danger),
                            ));
                        }
                    }

                    ListItem::new(Line::from(spans))
                }
            }
        })
        .collect();

    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(t.palette.divider))
        .title(Span::styled(
            " Files ",
            Style::default()
                .fg(t.palette.muted)
                .add_modifier(Modifier::BOLD),
        ));

    let total_lines = items.len();

    // Calculate visible viewport (area height minus borders)
    let viewport_height = area.height.saturating_sub(2) as usize;

    // Apply scroll offset - skip `scroll` items from the start
    let visible_items: Vec<ListItem> = items
        .into_iter()
        .skip(scroll)
        .take(viewport_height)
        .collect();

    f.render_widget(List::new(visible_items).block(block), area);

    total_lines
}

/// Simpler flat file list (no directory grouping) with status and stats.
///
/// Use this when you want a compact list without tree structure.
#[allow(dead_code)]
pub fn file_list_flat(
    f: &mut ratatui::Frame,
    files: &[FileEntry],
    selected: usize,
    area: Rect,
    show_stats: bool,
) {
    let t = theme::current();

    let items: Vec<ListItem> = files
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let is_selected = i == selected;

            // Status color
            let status_color = match file.status {
                'M' | '~' => t.palette.warning,
                'A' | '+' => t.palette.success,
                'D' | '-' => t.palette.danger,
                'R' => t.palette.accent,
                '?' => t.palette.muted,
                _ => t.palette.text,
            };

            let mut spans = vec![
                Span::styled(
                    format!(" {} ", file.status),
                    Style::default().fg(status_color),
                ),
                Span::styled(
                    file.path.clone(),
                    if is_selected {
                        Style::default()
                            .fg(t.palette.primary)
                            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                    } else {
                        Style::default().fg(t.palette.text)
                    },
                ),
            ];

            // Add stats
            if show_stats && (file.added > 0 || file.deleted > 0) {
                spans.push(Span::raw("  "));
                if file.added > 0 {
                    spans.push(Span::styled(
                        format!("+{}", file.added),
                        Style::default().fg(t.palette.success),
                    ));
                }
                if file.deleted > 0 {
                    if file.added > 0 {
                        spans.push(Span::raw(" "));
                    }
                    spans.push(Span::styled(
                        format!("-{}", file.deleted),
                        Style::default().fg(t.palette.danger),
                    ));
                }
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(t.palette.divider))
        .title(Span::styled(
            " Files ",
            Style::default()
                .fg(t.palette.muted)
                .add_modifier(Modifier::BOLD),
        ));

    f.render_widget(List::new(items).block(block), area);
}
