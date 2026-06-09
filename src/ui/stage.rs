//! Interactive staging picker — `g stage`.
//!
//! Two modes share the same [`StageEntry`] / [`StageResult`] types:
//!
//! - [`run`] — full-screen ratatui TUI (default, alternate screen).
//! - [`run_inline`] — inline checkbox list (used when `prompt_mode = "inline"`).
//!   Delegates to [`super::inline::inline_multi_select`] so the file list
//!   renders into normal scrollback without taking over the screen.
//!
//! Presents every changed file (staged, unstaged, untracked) as a directory
//! tree with checkboxes.  Checking an item means "stage this"; unchecking
//! means "unstage this".  Already-staged files start pre-checked so a second
//! invocation lets the user refine their selection.
//!
//! # Key bindings
//!
//! | Key        | Action                                       |
//! |------------|----------------------------------------------|
//! | `j` / `↓` | Move cursor down                             |
//! | `k` / `↑` | Move cursor up                               |
//! | `Space`    | Toggle checked state (dir = all children)    |
//! | `d`        | Revert the file at cursor (tracked only)     |
//! | `Enter`    | Confirm and apply                            |
//! | `Esc` / `q`| Cancel (no changes made)                     |
//!
//! # Screens
//!
//! - **Tree screen** — the main file tree picker.
//! - **Revert confirm screen** — shown when `d` is pressed (unless
//!   `[stage] confirm_revert = false`).

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Widget};
use ratatui_cheese::fieldset::{Fieldset, FieldsetFill, FieldsetStyles};
use ratatui_cheese::help::{Binding, Help, HelpStyles};

use super::render::indent;
use super::theme;

// ─── Public data types ────────────────────────────────────────────────────────

/// A single changed file parsed from `git status --porcelain`.
#[derive(Debug, Clone)]
pub struct StageEntry {
    /// Relative path from the repo root.
    pub path: String,
    /// Index (staged) status character from porcelain column 1.
    pub x: String,
    /// Working-tree status character from porcelain column 2.
    pub y: String,
    /// Whether the file currently has staged changes in the index.
    pub is_staged: bool,
    /// Whether the file is untracked (`??`).
    pub is_untracked: bool,
}

/// Changes to apply after the user confirms.
#[derive(Debug)]
pub struct StageResult {
    /// Paths to stage (`git add`).
    pub to_stage: Vec<String>,
    /// Paths to unstage (`git restore --staged`).
    pub to_unstage: Vec<String>,
    /// Paths to revert in the working tree (`git restore`).
    pub to_revert: Vec<String>,
}

// ─── Internal: flat tree model ────────────────────────────────────────────────

/// Whether a node's checkbox is on, off, or partially on (directory with mixed children).
#[derive(Debug, Clone, PartialEq)]
enum CheckState {
    Unchecked,
    Partial,
    Checked,
}

/// A flattened tree node — either a directory header or a file leaf.
#[derive(Debug, Clone)]
struct FlatNode {
    /// Nesting depth (0 = root).
    depth: usize,
    /// `true` for directory group headers, `false` for files.
    is_dir: bool,
    /// The filename or directory name (last path component).
    display_name: String,
    /// Full relative path from repo root.
    path: String,
    // File-only fields (empty for directories):
    x: String,
    y: String,
    is_staged: bool,
    is_untracked: bool,
    // Mutable user-selection state:
    checked: CheckState,
}

// ─── Internal: screen mode ────────────────────────────────────────────────────

enum ScreenMode {
    Tree,
    RevertConfirm {
        path: String,
        /// `true` = "Yes" button highlighted.
        confirm_yes: bool,
    },
}

// ─── Public entry point ───────────────────────────────────────────────────────

/// Run the full-screen staging TUI.
///
/// Returns `None` when the user cancels (`Esc` / `q`).
/// Returns `Some(StageResult)` when the user confirms; the caller is
/// responsible for running `git add`, `git restore --staged`, and
/// `git restore` as appropriate.
///
/// `confirm_revert` mirrors `[stage] confirm_revert` from config.
pub fn run(entries: Vec<StageEntry>, confirm_revert: bool) -> Option<StageResult> {
    if entries.is_empty() {
        return None;
    }

    let mut flat = build_flat_tree(&entries);
    if flat.is_empty() {
        return None;
    }

    let mut cursor = 0usize;
    let mut mode = ScreenMode::Tree;
    let mut reverted: Vec<String> = Vec::new();

    let mut terminal = ratatui::init();

    let result = loop {
        let n = flat.len();
        if n == 0 {
            // All files reverted — nothing left to stage.
            break Some(StageResult {
                to_stage: vec![],
                to_unstage: vec![],
                to_revert: reverted,
            });
        }
        cursor = cursor.min(n - 1);

        let _ = terminal.draw(|f| match &mode {
            ScreenMode::Tree => draw_tree(f, &flat, cursor),
            ScreenMode::RevertConfirm { path, confirm_yes } => {
                draw_revert_confirm(f, path, *confirm_yes)
            }
        });

        match event::read() {
            Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => match &mode {
                ScreenMode::Tree => match k.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        cursor = (cursor + 1).min(n - 1);
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        cursor = cursor.saturating_sub(1);
                    }
                    KeyCode::Char(' ') => {
                        toggle_node(&mut flat, cursor);
                    }
                    KeyCode::Char('d') => {
                        let node = &flat[cursor];
                        if node.is_dir {
                            // No revert for directories — skip silently.
                        } else if node.is_untracked {
                            // Untracked files have no previous state to restore.
                            // Ignore — user should use `g stage` to simply not stage them.
                        } else {
                            let path = node.path.clone();
                            if confirm_revert {
                                mode = ScreenMode::RevertConfirm {
                                    path,
                                    confirm_yes: false,
                                };
                            } else {
                                // Skip popup — record for immediate revert.
                                reverted.push(path.clone());
                                flat.retain(|n| {
                                    n.path != path && !n.path.starts_with(&format!("{}/", path))
                                });
                            }
                        }
                    }
                    KeyCode::Enter => {
                        break Some(collect_result(&flat, reverted));
                    }
                    KeyCode::Esc | KeyCode::Char('q') => break None,
                    _ => {}
                },

                ScreenMode::RevertConfirm { path, confirm_yes } => {
                    let path = path.clone();
                    let yes = *confirm_yes;
                    match k.code {
                        // Toggle Yes / No with arrow keys or Tab.
                        KeyCode::Left
                        | KeyCode::Right
                        | KeyCode::Tab
                        | KeyCode::Char('h')
                        | KeyCode::Char('l') => {
                            mode = ScreenMode::RevertConfirm {
                                path,
                                confirm_yes: !yes,
                            };
                        }
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            // Immediate yes
                            reverted.push(path.clone());
                            flat.retain(|n| n.path != path);
                            update_all_dir_states(&mut flat);
                            mode = ScreenMode::Tree;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            mode = ScreenMode::Tree;
                        }
                        KeyCode::Enter => {
                            if yes {
                                reverted.push(path.clone());
                                flat.retain(|n| n.path != path);
                                update_all_dir_states(&mut flat);
                            }
                            mode = ScreenMode::Tree;
                        }
                        _ => {}
                    }
                }
            },
            _ => {}
        }
    };

    ratatui::restore();
    result
}

// ─── Tree building ────────────────────────────────────────────────────────────

fn build_flat_tree(entries: &[StageEntry]) -> Vec<FlatNode> {
    // Sort entries alphabetically by full path so the tree is consistent.
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));

    let mut result: Vec<FlatNode> = Vec::new();
    // Track which directory headers have already been inserted.
    let mut seen_dirs: std::collections::HashSet<String> = std::collections::HashSet::new();

    for entry in &sorted {
        let parts: Vec<&str> = entry.path.split('/').collect();

        // Insert ancestor directory headers as needed.
        for ancestor_depth in 0..parts.len().saturating_sub(1) {
            let dir_path = parts[..=ancestor_depth].join("/");
            if !seen_dirs.contains(&dir_path) {
                seen_dirs.insert(dir_path.clone());
                result.push(FlatNode {
                    depth: ancestor_depth,
                    is_dir: true,
                    display_name: parts[ancestor_depth].to_string(),
                    path: dir_path,
                    x: String::new(),
                    y: String::new(),
                    is_staged: false,
                    is_untracked: false,
                    checked: CheckState::Unchecked, // updated after all files are inserted
                });
            }
        }

        // Insert the file leaf.
        let depth = parts.len() - 1;
        result.push(FlatNode {
            depth,
            is_dir: false,
            display_name: parts[depth].to_string(),
            path: entry.path.clone(),
            x: entry.x.clone(),
            y: entry.y.clone(),
            is_staged: entry.is_staged,
            is_untracked: entry.is_untracked,
            // Pre-check files that are already staged so re-entering the picker
            // shows the current state and the user can adjust their selection.
            checked: if entry.is_staged {
                CheckState::Checked
            } else {
                CheckState::Unchecked
            },
        });
    }

    // Derive directory checked states from their file children.
    update_all_dir_states(&mut result);
    result
}

/// Recompute the `checked` state of every directory from its file descendants.
fn update_all_dir_states(nodes: &mut [FlatNode]) {
    for i in 0..nodes.len() {
        if !nodes[i].is_dir {
            continue;
        }
        let dir_depth = nodes[i].depth;

        let mut total = 0usize;
        let mut checked_count = 0usize;

        for node in nodes.iter().skip(i + 1) {
            // Stop when we leave this directory's subtree.
            if node.depth <= dir_depth {
                break;
            }
            if !node.is_dir {
                total += 1;
                if node.checked == CheckState::Checked {
                    checked_count += 1;
                }
            }
        }

        nodes[i].checked = match (checked_count, total) {
            (0, _) => CheckState::Unchecked,
            (c, t) if c == t => CheckState::Checked,
            _ => CheckState::Partial,
        };
    }
}

// ─── Toggle logic ─────────────────────────────────────────────────────────────

fn toggle_node(nodes: &mut [FlatNode], idx: usize) {
    if nodes[idx].is_dir {
        // For a directory: if currently checked or partial → uncheck all; else → check all.
        let new_state = match nodes[idx].checked {
            CheckState::Checked => CheckState::Unchecked,
            _ => CheckState::Checked,
        };
        let dir_depth = nodes[idx].depth;
        // Apply to all file descendants.
        for node in nodes.iter_mut().skip(idx + 1) {
            if node.depth <= dir_depth {
                break;
            }
            if !node.is_dir {
                node.checked = new_state.clone();
            }
        }
    } else {
        nodes[idx].checked = match nodes[idx].checked {
            CheckState::Checked => CheckState::Unchecked,
            _ => CheckState::Checked,
        };
    }
    update_all_dir_states(nodes);
}

// ─── Collect result ───────────────────────────────────────────────────────────

fn collect_result(nodes: &[FlatNode], to_revert: Vec<String>) -> StageResult {
    let mut to_stage = Vec::new();
    let mut to_unstage = Vec::new();

    for node in nodes {
        if node.is_dir {
            continue;
        }
        match node.checked {
            CheckState::Checked if !node.is_staged => {
                // User wants it staged but it isn't yet.
                to_stage.push(node.path.clone());
            }
            CheckState::Unchecked if node.is_staged => {
                // User unchecked a file that was staged — unstage it.
                to_unstage.push(node.path.clone());
            }
            _ => {} // No change needed.
        }
    }

    StageResult {
        to_stage,
        to_unstage,
        to_revert,
    }
}

// ─── Rendering: tree screen ───────────────────────────────────────────────────

fn draw_tree(f: &mut ratatui::Frame, nodes: &[FlatNode], cursor: usize) {
    let area = f.area();
    let t = theme::current();

    // Layout: [header 1][summary+legend 1][blank 1][list fill][help 1]
    let chunks = Layout::vertical([
        Constraint::Length(1), // fieldset header
        Constraint::Length(1), // summary + legend
        Constraint::Length(1), // blank separator
        Constraint::Min(1),    // file tree
        Constraint::Length(1), // help bar
    ])
    .split(area);

    // Header fieldset.
    render_fieldset(f, "Stage Files", chunks[0]);

    // Summary + colour legend on the same row.
    //
    // Pattern: "  4/7 selected   M modified · A added · D deleted · ? untracked"
    // Each legend badge is colored with the same color applied to file names.
    let total: usize = nodes.iter().filter(|n| !n.is_dir).count();
    let checked: usize = nodes
        .iter()
        .filter(|n| !n.is_dir && n.checked == CheckState::Checked)
        .count();

    let legend = Line::from(vec![
        Span::raw(indent()),
        Span::styled(
            format!("{}/{} selected", checked, total),
            Style::default().fg(t.palette.muted),
        ),
        Span::styled("   ", Style::default()),
        Span::styled("M", Style::default().fg(t.palette.warning)),
        Span::styled(" modified", Style::default().fg(t.palette.muted)),
        Span::styled("  ·  ", Style::default().fg(t.palette.muted)),
        Span::styled("A", Style::default().fg(t.palette.success)),
        Span::styled(" added", Style::default().fg(t.palette.muted)),
        Span::styled("  ·  ", Style::default().fg(t.palette.muted)),
        Span::styled("D", Style::default().fg(t.palette.danger)),
        Span::styled(" deleted", Style::default().fg(t.palette.muted)),
        Span::styled("  ·  ", Style::default().fg(t.palette.muted)),
        Span::styled("?", Style::default().fg(t.palette.muted)),
        Span::styled(" untracked", Style::default().fg(t.palette.muted)),
        Span::styled("  ·  ", Style::default().fg(t.palette.muted)),
        Span::styled("U", Style::default().fg(t.palette.danger)),
        Span::styled(" conflict", Style::default().fg(t.palette.muted)),
    ]);
    f.render_widget(Paragraph::new(legend), chunks[1]);

    // File tree list.
    //
    // Rendered as a StatefulWidget with a ListState so ratatui automatically
    // scrolls the viewport to keep the cursor row visible.  The visual cursor
    // indicator (`>` + colour) is still drawn inside each ListItem by
    // `build_list_item`; the ListState only controls the scroll offset.
    let list_items: Vec<ListItem<'static>> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| build_list_item(node, i == cursor, t))
        .collect();
    let mut list_state = ListState::default().with_selected(Some(cursor));
    f.render_stateful_widget(List::new(list_items), chunks[3], &mut list_state);

    // Help bar.
    render_help(
        f,
        &[
            ("j/k", "move"),
            ("Space", "toggle"),
            ("d", "revert"),
            ("Enter", "confirm"),
            ("q", "cancel"),
        ],
        chunks[4],
    );
}

fn build_list_item(node: &FlatNode, is_cursor: bool, t: &super::theme::Theme) -> ListItem<'static> {
    // ── Status badge (always at the start, before cursor and checkbox) ─────────
    //
    // Layout of the fixed prefix:
    //   [outer_indent][X][Y][space][icon][space][cursor][space][checkbox][space]
    //
    // For files:  X = index status (green if staged, muted space otherwise)
    //             Y = working-tree status (yellow if changed, muted space otherwise)
    //             icon = ✎/✚/✖/➜/? colored to match the dominant status
    //
    // For dirs:   three muted spaces + muted bullet occupy the same width so
    //             columns stay aligned across mixed file/dir rows.
    //
    // IMPORTANT: every color here MUST use `Style::default().fg(ratatui_color)`.
    //            Never embed crossterm ANSI escape codes (from `paint()`) in
    //            Span content — ratatui treats Span text as literals.

    // Status badge: XY columns only — no icon.
    // X = index/staged status (green when staged), Y = working-tree status (yellow when changed).
    // Directories get two muted spaces to preserve column alignment.
    let (x_text, x_style, y_text, y_style) = if node.is_dir {
        (
            " ".to_string(),
            Style::default().fg(t.palette.muted),
            " ".to_string(),
            Style::default().fg(t.palette.muted),
        )
    } else {
        let (xt, xs) = if node.x != " " && node.x != "?" && node.x != "!" {
            (node.x.clone(), Style::default().fg(t.palette.success))
        } else {
            (" ".to_string(), Style::default().fg(t.palette.muted))
        };

        let (yt, ys) = if node.y != " " && node.y != "?" && node.y != "!" {
            (node.y.clone(), Style::default().fg(t.palette.warning))
        } else if node.is_untracked {
            ("?".to_string(), Style::default().fg(t.palette.muted))
        } else {
            (" ".to_string(), Style::default().fg(t.palette.muted))
        };

        (xt, xs, yt, ys)
    };

    // ── Dominant line color (applied to the file/dir name) ────────────────────
    //
    // Cursor overrides everything → primary color + bold.
    // Otherwise: status determines color (modification = yellow, new = green,
    // deleted = red, untracked = muted, directory = white bold, staged = green).
    let name_color = if is_cursor {
        t.palette.primary
    } else if node.is_dir {
        t.palette.text
    } else {
        line_color(&node.x, &node.y, t)
    };
    let name_modifier = if is_cursor || node.is_dir {
        Modifier::BOLD
    } else {
        Modifier::empty()
    };
    let name_style = Style::default().fg(name_color).add_modifier(name_modifier);

    // ── Checkbox ──────────────────────────────────────────────────────────────
    let (checkbox, checkbox_style) = if node.is_dir {
        let (sym, col) = match node.checked {
            CheckState::Checked => ("[✓]", t.palette.success),
            CheckState::Partial => ("[-]", t.palette.warning),
            CheckState::Unchecked => ("[ ]", t.palette.muted),
        };
        (sym, Style::default().fg(col))
    } else {
        let (sym, col) = match node.checked {
            CheckState::Checked => ("[✓]", t.palette.success),
            _ => ("[ ]", t.palette.muted),
        };
        (sym, Style::default().fg(col))
    };

    // ── Cursor indicator ──────────────────────────────────────────────────────
    let cursor_str = if is_cursor { ">" } else { " " };
    let cursor_style = if is_cursor {
        Style::default()
            .fg(t.palette.primary)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(t.palette.muted)
    };

    // ── Tree indent (2 spaces per depth) ─────────────────────────────────────
    let tree_indent = "  ".repeat(node.depth);
    let dir_suffix = if node.is_dir { "/" } else { "" };

    // ── Assemble spans ────────────────────────────────────────────────────────
    //
    // [OUTER_indent][X][Y][space][cursor][space][checkbox][space][tree_indent][name]
    Line::from(vec![
        Span::styled(indent(), Style::default()),
        Span::styled(x_text, x_style), // index status (green / muted)
        Span::styled(y_text, y_style), // working-tree status (yellow / muted)
        Span::raw("  "),
        Span::styled(cursor_str, cursor_style), // > or space
        Span::raw(" "),
        Span::styled(checkbox, checkbox_style), // [✓] / [-] / [ ]
        Span::raw(" "),
        Span::raw(tree_indent), // depth indent (2 spaces × depth)
        Span::styled(
            format!("{}{}", node.display_name, dir_suffix),
            name_style, // color = status color
        ),
    ])
    .into()
}

/// Derive the dominant line color for a file based on its git status.
fn line_color(x: &str, y: &str, t: &super::theme::Theme) -> ratatui::style::Color {
    // Conflict → danger (takes priority over everything)
    if x == "U" || y == "U" {
        return t.palette.danger;
    }
    // Deleted → danger
    if x == "D" || y == "D" {
        return t.palette.danger;
    }
    // Untracked → muted
    if x == "?" && y == "?" {
        return t.palette.muted;
    }
    // Staged change → derive from index column
    if x != " " && x != "?" && x != "!" {
        return match x {
            "A" | "C" => t.palette.success, // added / copied → green
            "M" => t.palette.warning,       // modified → yellow
            "R" => t.palette.primary,       // renamed → cyan
            _ => t.palette.text,
        };
    }
    // Working-tree only change → derive from Y column
    match y {
        "M" => t.palette.warning, // modified → yellow
        "A" => t.palette.success, // added → green (shouldn't happen in porcelain v1 but handle it)
        _ => t.palette.text,
    }
}

// ─── Rendering: revert confirm screen ────────────────────────────────────────

// ─── Rendering: revert confirm screen ────────────────────────────────────────

fn draw_revert_confirm(f: &mut ratatui::Frame, path: &str, confirm_yes: bool) {
    let area = f.area();
    let t = theme::current();

    let chunks = Layout::vertical([
        Constraint::Length(1), // fieldset header
        Constraint::Length(1), // blank
        Constraint::Length(1), // warning line
        Constraint::Length(1), // detail line
        Constraint::Length(1), // blank
        Constraint::Length(1), // Yes / No buttons
        Constraint::Min(0),    // filler
        Constraint::Length(1), // help
    ])
    .split(area);

    // Header: reuse the slash fieldset.
    render_fieldset(
        f,
        &format!(
            "Revert  {}",
            truncate_path(path, (area.width as usize).saturating_sub(20))
        ),
        chunks[0],
    );

    // Warning text.
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(indent()),
            Span::styled(
                "This will discard all local changes and cannot be undone.",
                Style::default().fg(t.palette.warning),
            ),
        ])),
        chunks[2],
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(indent()),
            Span::styled(
                format!("File: {}", path),
                Style::default()
                    .fg(t.palette.muted)
                    .add_modifier(Modifier::DIM),
            ),
        ])),
        chunks[3],
    );

    // Yes / No buttons.
    let yes_style = if confirm_yes {
        Style::default()
            .fg(t.palette.danger)
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default().fg(t.palette.muted)
    };
    let no_style = if !confirm_yes {
        Style::default()
            .fg(t.palette.success)
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default().fg(t.palette.muted)
    };
    let buttons = Line::from(vec![
        Span::raw(indent()),
        Span::raw("  "),
        Span::styled("  Yes, revert  ", yes_style),
        Span::raw("   "),
        Span::styled("  No, keep changes  ", no_style),
    ]);
    f.render_widget(Paragraph::new(buttons), chunks[5]);

    render_help(
        f,
        &[
            ("y/n", "choose"),
            ("←/→", "toggle"),
            ("Enter", "confirm"),
            ("Esc", "cancel"),
        ],
        chunks[7],
    );
}

/// Truncate a path from the left if it exceeds `max_len` visible chars.
fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else {
        format!("…{}", &path[path.len() - max_len.saturating_sub(1)..])
    }
}

// ─── Shared rendering helpers ─────────────────────────────────────────────────

fn render_fieldset(f: &mut ratatui::Frame, title: &str, area: Rect) {
    let t = theme::current();
    let rule_style = Style::default().fg(t.palette.divider);
    let title_style = Style::default()
        .fg(t.palette.muted)
        .add_modifier(Modifier::BOLD);

    // Cap the fieldset width at 100 columns so it doesn't fill the entire
    // terminal on very wide displays — consistent with how the static
    // `print_fieldset` is bounded by the 55-char subject cap in log output.
    let capped_area = Rect {
        width: area.width.min(100),
        ..area
    };

    let padded = format!("  {}  ", title);
    Fieldset::new()
        .title(padded.as_str())
        .fill(FieldsetFill::Slash)
        .top_alignment(Alignment::Left)
        .styles(FieldsetStyles {
            title: title_style,
            rule: rule_style,
        })
        .render(capped_area, f.buffer_mut());
}

fn render_help(f: &mut ratatui::Frame, items: &[(&str, &str)], area: Rect) {
    let t = theme::current();
    let key_style = Style::default().fg(t.palette.text);
    let desc_style = Style::default().fg(t.palette.muted);
    let styles = HelpStyles {
        ellipsis: desc_style,
        short_key: key_style,
        short_desc: desc_style,
        short_separator: desc_style,
        full_key: key_style,
        full_desc: desc_style,
        full_separator: desc_style,
    };
    let bindings: Vec<Binding> = items.iter().map(|(k, d)| Binding::new(*k, *d)).collect();
    Help::default()
        .styles(styles)
        .bindings(bindings)
        .render(area, f.buffer_mut());
}

// ─── Inline stage (non-fullscreen) ───────────────────────────────────────────

/// Run the staging picker in inline mode (no alternate screen).
///
/// Builds an [`super::interactive::SelectOption`] list from `entries`, marks
/// currently-staged files as pre-selected, and delegates to
/// [`super::inline::inline_multi_select`].  The user toggles files with
/// `Space`, navigates with `j`/`k`, and confirms with `Enter`.
///
/// Returns `None` on cancel (same contract as [`run`]).
pub fn run_inline(entries: Vec<StageEntry>, _confirm_revert: bool) -> Option<StageResult> {
    use super::inline::inline_multi_select;
    use super::interactive::SelectOption;

    if entries.is_empty() {
        return None;
    }

    // Build display options — one per file, pre-check currently staged ones.
    let pre_selected: Vec<bool> = entries.iter().map(|e| e.is_staged).collect();
    let options: Vec<SelectOption> = entries
        .iter()
        .map(|e| {
            // Status badge: use the dominant column for the label.
            let status = if e.is_untracked {
                "?".to_string()
            } else if e.x != " " && e.x != "?" {
                e.x.clone() // staged change visible in index
            } else {
                e.y.clone() // working-tree change
            };

            let desc = match status.as_str() {
                "M" => "modified",
                "A" => "added",
                "D" => "deleted",
                "R" => "renamed",
                "C" => "copied",
                "?" => "untracked",
                "U" => "conflict",
                _ => "",
            };

            SelectOption::with_description(format!("{}  {}", status, e.path), desc)
        })
        .collect();

    let selected_indices = inline_multi_select("Stage Files", &options, &pre_selected);

    // Build a boolean mask of what the user wants staged.
    let want_staged: Vec<bool> = (0..entries.len())
        .map(|i| selected_indices.contains(&i))
        .collect();

    let mut to_stage = Vec::new();
    let mut to_unstage = Vec::new();

    for (entry, &want) in entries.iter().zip(want_staged.iter()) {
        if want && !entry.is_staged {
            to_stage.push(entry.path.clone());
        } else if !want && entry.is_staged {
            to_unstage.push(entry.path.clone());
        }
    }

    Some(StageResult {
        to_stage,
        to_unstage,
        to_revert: vec![], // revert is not surfaced in the inline picker
    })
}
