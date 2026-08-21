//! Mode 2 — inline widget rendering.
//!
//! Widgets here produce structured multi-cell output: tables, trees, section
//! dividers, commit log entries.  Where possible they delegate to ratatui-cheese
//! widgets rendered into an in-memory [`ratatui::buffer::Buffer`] which is then
//! flushed to stdout via [`super::render::print_buffer_row`].  This avoids
//! creating a full `Terminal` instance for each output line while still using
//! the ratatui widget library as intended.
//!
//! # ratatui-cheese widgets used
//!
//! - [`ratatui_cheese::fieldset::Fieldset`] with `FieldsetFill::Slash` for
//!   all top-level section dividers (`/////  Title  /////…`).

use std::fmt::Write as FmtWrite;

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::widgets::Widget;
use ratatui_cheese::fieldset::{Fieldset, FieldsetFill, FieldsetStyles};

use super::render::{indent, paint, paint_bold, paint_dim, print_buffer_row, terminal_width};
use super::theme;

// ─── Fieldset (dot divider) ───────────────────────────────────────────────────

/// Print a top-level section divider with a dot fill and left-aligned title.
///
/// Uses `ratatui_cheese::fieldset::Fieldset` with `FieldsetFill::Dot`.
/// The widget is rendered to an in-memory `Buffer` and flushed to stdout via
/// `print_buffer_row` — no `Terminal` instance is required.
///
/// ```text
/// •••••  Title  •••••••••••••••••••••••••••••••••••••••••••••••••••
/// ```
pub fn print_fieldset(title: &str) {
    let t = theme::current();
    let width = terminal_width().saturating_sub(indent().len()) as u16;
    let area = Rect {
        x: 0,
        y: 0,
        width,
        height: 1,
    };
    let mut buffer = Buffer::empty(area);

    // ratatui Style uses ratatui Color directly (not ct_color which is crossterm)
    let rule_style = Style::default().fg(t.palette.divider);
    let title_style = Style::default()
        .fg(t.palette.accent)
        .add_modifier(ratatui::style::Modifier::BOLD);

    // ratatui-cheese Fieldset::title takes &str; styling comes from FieldsetStyles.
    // We use a padded title string (" Title ") so there's breathing room around
    // the text against the dot fill.
    let padded = format!("{} ", title);
    Fieldset::new()
        .title(padded.as_str())
        .fill(FieldsetFill::Dot)
        .top_alignment(Alignment::Left)
        .styles(FieldsetStyles {
            title: title_style,
            rule: rule_style,
        })
        .render(area, &mut buffer);

    print!("{}", indent());
    print_buffer_row(&buffer, 0);
}

// ─── Branch / stack markers ───────────────────────────────────────────────────

/// Return the filled (`◉`) or hollow (`◯`) branch marker colored by state.
pub fn branch_marker(is_current: bool) -> String {
    let t = theme::current();
    if is_current {
        paint_bold(t.icons.current, t.palette.success)
    } else {
        paint(t.icons.other, t.palette.muted)
    }
}

/// Return a branch name colored by whether it is the current branch.
pub fn branch_name_colored(name: &str, is_current: bool) -> String {
    use super::print::{paint_text, success_bold};
    if is_current {
        success_bold(name)
    } else {
        paint_text(name)
    }
}

// ─── Git colour helpers ───────────────────────────────────────────────────────

/// Color a commit hash (yellow, dim).
pub fn color_hash(hash: &str) -> String {
    paint_dim(hash, theme::current().palette.warning)
}

/// Color a branch name by its type.
///
/// - Remote branches (`origin/…`, `upstream/…`) → danger bold.
/// - `HEAD` → primary bold.
/// - Local branches → success bold.
pub fn color_branch(name: &str) -> String {
    use super::print::{danger_bold, primary_bold, success_bold};
    if name.starts_with("origin/") || name.starts_with("upstream/") {
        danger_bold(name)
    } else if name == "HEAD" {
        primary_bold(name)
    } else {
        success_bold(name)
    }
}

/// Color a ref decoration string (HEAD, tags, remotes, local branches).
pub fn color_ref(r: &str) -> String {
    use super::print::{danger, primary_bold, success_bold, warning_bold};
    if r.contains("HEAD") {
        primary_bold(r)
    } else if r.starts_with("tag:") {
        warning_bold(r)
    } else if r.contains('/') {
        danger(r)
    } else {
        success_bold(r)
    }
}

/// Color an author name in the primary accent color.
pub fn color_author(name: &str) -> String {
    use super::print::primary;
    primary(name)
}

/// Color a date string in the muted color.
pub fn color_date(date: &str) -> String {
    use super::print::muted;
    muted(date)
}

/// Color a commit subject, highlighting Conventional Commit type prefixes.
pub fn color_subject(subject: &str) -> String {
    use super::print::text_bold;
    let t = theme::current();
    if let Some(idx) = subject.find(':') {
        let prefix = &subject[..idx];
        let rest = &subject[idx..];
        let colored_prefix = if prefix.starts_with("feat") {
            paint_bold(prefix, t.palette.cc_feat)
        } else if prefix.starts_with("fix") {
            paint_bold(prefix, t.palette.cc_fix)
        } else if prefix.starts_with("docs") {
            paint_bold(prefix, t.palette.cc_docs)
        } else if prefix.starts_with("refactor") {
            paint_bold(prefix, t.palette.cc_refactor)
        } else if prefix.starts_with("perf") {
            paint_bold(prefix, t.palette.cc_perf)
        } else if prefix.starts_with("test") {
            paint_bold(prefix, t.palette.cc_test)
        } else if prefix.starts_with("chore")
            || prefix.starts_with("build")
            || prefix.starts_with("ci")
        {
            paint_bold(prefix, t.palette.cc_chore)
        } else if prefix.starts_with("revert") {
            paint_dim(prefix, t.palette.cc_revert)
        } else {
            text_bold(prefix)
        };
        format!("{}{}", colored_prefix, paint(rest, t.palette.text))
    } else {
        paint(subject, t.palette.text)
    }
}

/// Render `+N` in the success color.
pub fn color_added(n: i64) -> String {
    use super::print::success;
    success(&format!("+{}", n))
}

/// Render `-N` in the danger color.
pub fn color_deleted(n: i64) -> String {
    use super::print::danger;
    danger(&format!("-{}", n))
}

// ─── Status icons ─────────────────────────────────────────────────────────────

/// Map a git porcelain status code to `(icon, colored_code)`.
pub fn status_icon(code: &str) -> (&'static str, String) {
    use super::print::{danger_bold, muted, primary, primary_bold, success_bold, warning_bold};
    let t = theme::current();
    match code {
        "A" | "AA" => (t.icons.added, success_bold("A")),
        "M" | "MM" => (t.icons.modified, warning_bold("M")),
        "D" | "DD" => (t.icons.deleted, danger_bold("D")),
        "R" | "RR" => (t.icons.renamed, primary_bold("R")),
        "C" | "CC" => ("⊕", primary("C")),
        "U" | "UU" => ("⚡", danger_bold("U")),
        "?" => ("?", muted("?")),
        "!" => ("!", muted("!")),
        _ => ("·", muted(code)),
    }
}

// ─── Diff stat bar ────────────────────────────────────────────────────────────

/// Render a fixed-width bar showing added-vs-deleted proportions.
///
/// Green `█` blocks represent additions; red `█` blocks represent deletions.
/// Returns an empty string when both counts are zero.
pub fn render_stat_bar(added: usize, deleted: usize, width: usize) -> String {
    use super::print::{danger, success};
    let total = added + deleted;
    if total == 0 {
        return String::new();
    }
    let add_blocks = (added * width / total).max(if added > 0 { 1 } else { 0 });
    let del_blocks = (width - add_blocks).min(deleted.min(width));
    format!(
        "{}{}",
        success(&"█".repeat(add_blocks)),
        danger(&"█".repeat(del_blocks)),
    )
}

// ─── Ref decoration formatter ─────────────────────────────────────────────────

/// Format git ref decorations (`HEAD -> main, origin/main`) into colored badges.
///
/// Returns an empty string when `refs_str` is blank.
pub fn format_refs(refs_str: &str) -> String {
    use super::print::{muted, primary_bold};
    if refs_str.trim().is_empty() {
        return String::new();
    }
    let parts: Vec<&str> = refs_str.split(',').map(str::trim).collect();
    let formatted: Vec<String> = parts
        .iter()
        .filter(|r| !r.is_empty())
        .map(|r| {
            if r.starts_with("HEAD ->") {
                let branch = r.trim_start_matches("HEAD ->").trim();
                format!(
                    "{} {} {}",
                    primary_bold("HEAD →"),
                    muted(""),
                    color_branch(branch)
                )
            } else {
                color_ref(r)
            }
        })
        .collect();
    if formatted.is_empty() {
        return String::new();
    }
    format!(
        " {} {} {}",
        muted("("),
        formatted.join(&muted(" · ")),
        muted(")")
    )
}

// ─── Ahead / behind formatter ────────────────────────────────────────────────

/// Format ahead/behind commit counts into a compact colored string.
pub fn format_ahead_behind(ahead: usize, behind: usize) -> String {
    use super::print::{danger, muted, success};
    let t = theme::current();
    match (ahead, behind) {
        (0, 0) => muted("up to date"),
        (a, 0) => format!(
            "{} {}",
            success(t.icons.ahead),
            success(&format!("{} ahead", a))
        ),
        (0, b) => format!(
            "{} {}",
            danger(t.icons.behind),
            danger(&format!("{} behind", b))
        ),
        (a, b) => format!(
            "{} {}  {} {}",
            success(t.icons.ahead),
            success(&format!("{} ahead", a)),
            danger(t.icons.behind),
            danger(&format!("{} behind", b))
        ),
    }
}

// ─── Table formatter ──────────────────────────────────────────────────────────

/// A columnar table renderer with ANSI-aware column width tracking.
///
/// Uses `console::measure_text_width` so that ANSI escape codes inside cells
/// don't distort column alignment.
pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    col_widths: Vec<usize>,
    /// Per-column shrink policy used when the table would overflow the terminal.
    ///
    /// `None` = protected: the column is never truncated (e.g. a branch name).
    /// `Some(min)` = flexible: the column may be truncated with an ellipsis down
    /// to `min` visible columns to reclaim horizontal space.
    flex: Vec<Option<usize>>,
}

impl Table {
    /// Create a new table with the given header labels.
    pub fn new(headers: Vec<&str>) -> Self {
        let col_widths: Vec<usize> = headers
            .iter()
            .map(|h| console::measure_text_width(h))
            .collect();
        let flex = vec![None; col_widths.len()];
        Self {
            headers: headers.into_iter().map(String::from).collect(),
            rows: vec![],
            col_widths,
            flex,
        }
    }

    /// Mark column `col` as flexible: when the full table would overflow the
    /// terminal, this column may be truncated (with a trailing `…`) down to
    /// `min_width` visible columns to make room for protected columns.
    ///
    /// Columns are protected by default, so tables that never call this keep
    /// their previous grow-to-fit behaviour.
    pub fn set_flexible(mut self, col: usize, min_width: usize) -> Self {
        if let Some(slot) = self.flex.get_mut(col) {
            *slot = Some(min_width);
        }
        self
    }

    /// Append a data row, expanding column widths as needed.
    pub fn add_row(&mut self, row: Vec<String>) {
        for (i, cell) in row.iter().enumerate() {
            let w = console::measure_text_width(cell);
            if i < self.col_widths.len() {
                self.col_widths[i] = self.col_widths[i].max(w);
            }
        }
        self.rows.push(row);
    }

    /// Compute the effective per-column widths for the current terminal.
    ///
    /// Starts from the natural (grow-to-fit) widths. If the row would overflow
    /// the terminal, flexible columns are trimmed one visible column at a time,
    /// always from the currently-widest flexible column, until the table fits
    /// or no flexible slack remains. Protected columns are never reduced.
    fn fit_widths(&self) -> Vec<usize> {
        let mut widths = self.col_widths.clone();
        let n = widths.len();
        if n == 0 {
            return widths;
        }

        let gap_w = console::measure_text_width(theme::current().spacing.col_gap);
        let indent_w = console::measure_text_width(indent());
        let avail = terminal_width();
        let overhead = indent_w + gap_w * n.saturating_sub(1);
        let natural: usize = widths.iter().sum::<usize>() + overhead;
        if natural <= avail {
            return widths;
        }

        let mut overflow = natural - avail;
        while overflow > 0 {
            // Pick the widest column that still has flexible slack.
            let mut victim: Option<usize> = None;
            let mut victim_w = 0;
            for (i, policy) in self.flex.iter().enumerate() {
                if let Some(min) = *policy {
                    if widths[i] > min && widths[i] > victim_w {
                        victim_w = widths[i];
                        victim = Some(i);
                    }
                }
            }
            match victim {
                Some(i) => {
                    widths[i] -= 1;
                    overflow -= 1;
                }
                // No flexible slack left; the row will overflow rather than
                // truncate a protected column (e.g. a branch name).
                None => break,
            }
        }
        widths
    }

    /// Print the table to stdout: headers, a `─` divider, then each row.
    pub fn print(&self) {
        use super::print::{muted, text_bold};
        let t = theme::current();
        let gap = t.spacing.col_gap;
        let hline = t.borders.horizontal;
        let widths = self.fit_widths();
        let pad_cell = |cell: &str, col: usize| -> String {
            let target = widths.get(col).copied().unwrap_or(0);
            let cell = truncate(cell, target);
            let vis = console::measure_text_width(&cell);
            format!("{}{}", cell, " ".repeat(target.saturating_sub(vis)))
        };

        let header_cells: Vec<String> = self
            .headers
            .iter()
            .enumerate()
            .map(|(i, h)| pad_cell(&text_bold(h), i))
            .collect();
        println!("{}{}", indent(), header_cells.join(gap));

        let divider: Vec<String> = widths
            .iter()
            .map(|w| muted(&hline.to_string().repeat(*w)))
            .collect();
        println!("{}{}", indent(), divider.join(gap));

        for row in &self.rows {
            let cells: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(i, cell)| pad_cell(cell, i))
                .collect();
            println!("{}{}", indent(), cells.join(gap));
        }
    }
}

// ─── Commit log layout ────────────────────────────────────────────────────────

/// Calculate the optimal subject column width for [`CommitEntry::render`].
///
/// Accounts for all fixed-width columns that appear on the same line:
///
/// ```text
/// [graph_prefix][SP][hash 7][SP][subject N][SP×2][author 20][SP×2][date 14]
/// ```
///
/// The result adapts to the terminal width on narrow terminals (below ~100 cols)
/// but is capped at `MAX_SUBJECT` on wider terminals so the log stays compact and
/// readable rather than spreading a single line across the whole screen.
///
/// `show_graph` adds an 8-char budget for the ASCII branch graph.
pub fn commit_subject_width(show_graph: bool) -> usize {
    // space(1) + hash(7) + space(1) + sep(2) + author(20) + sep(2) + date-budget(14)
    const FIXED: usize = 1 + 7 + 1 + 2 + 20 + 2 + 14;
    const GRAPH_BUDGET: usize = 8;
    /// Maximum subject column width — keeps lines compact on wide terminals.
    /// Matches the conventional-commit subject length recommendation (72 chars)
    /// but trimmed to 55 so the overall log line stays under ~100 columns.
    const MAX_SUBJECT: usize = 55;

    let overhead = FIXED + if show_graph { GRAPH_BUDGET } else { 0 };
    terminal_width()
        .saturating_sub(overhead)
        .clamp(30, MAX_SUBJECT)
}

// ─── Commit entry ─────────────────────────────────────────────────────────────

/// A single git log entry ready to be rendered as a terminal line.
pub struct CommitEntry {
    /// Short (7-char) hash.
    pub hash: String,
    /// Commit subject line.
    pub subject: String,
    /// Author name.
    pub author: String,
    /// Relative date string.
    pub date: String,
    /// Raw ref decorations from `%D`.
    pub refs: String,
    /// ASCII graph prefix from `git log --graph`.
    pub graph_prefix: String,
}

impl CommitEntry {
    /// Render the entry as a single colored terminal line.
    ///
    /// `max_subject` is the column width reserved for the subject; shorter
    /// subjects are padded so all columns align.
    pub fn render(&self, max_subject: usize) -> String {
        let mut out = String::new();

        if !self.graph_prefix.is_empty() {
            write!(out, "{}", colorize_graph(&self.graph_prefix)).ok();
        }

        write!(out, " {} ", color_hash(&self.hash)).ok();

        let subject = truncate(&self.subject, max_subject);
        let colored_subject = color_subject(&subject);
        let subject_vis = console::measure_text_width(&colored_subject);
        write!(
            out,
            "{}{}",
            colored_subject,
            " ".repeat(max_subject.saturating_sub(subject_vis))
        )
        .ok();

        let author_max = 20;
        let author = truncate(&self.author, author_max);
        let colored_author = color_author(&author);
        let author_vis = console::measure_text_width(&colored_author);
        write!(
            out,
            "  {}{}",
            colored_author,
            " ".repeat(author_max.saturating_sub(author_vis))
        )
        .ok();

        write!(out, "  {}", color_date(&self.date)).ok();

        if !self.refs.trim().is_empty() {
            write!(out, " {}", format_refs(&self.refs)).ok();
        }

        out
    }
}

/// Truncate `s` to at most `max` visible characters, appending `…` if needed.
fn truncate(s: &str, max: usize) -> String {
    if console::measure_text_width(s) > max {
        let t = console::truncate_str(s, max.saturating_sub(1), "");
        format!("{}…", t)
    } else {
        s.to_string()
    }
}

// ─── Graph colorizer ──────────────────────────────────────────────────────────

/// Apply cycle-colors to the ASCII branch graph produced by `git log --graph`.
pub fn colorize_graph(graph: &str) -> String {
    let colors = [
        "\x1b[33m", // yellow
        "\x1b[32m", // green
        "\x1b[34m", // blue
        "\x1b[35m", // magenta
        "\x1b[36m", // cyan
    ];
    let reset = "\x1b[0m";
    let mut result = String::new();
    for (i, ch) in graph.chars().enumerate() {
        match ch {
            '*' => result.push_str(&format!("{}{}{}", colors[0], ch, reset)),
            '|' => {
                let idx = (i / 2) % colors.len();
                result.push_str(&format!("{}{}{}", colors[idx], ch, reset));
            }
            '/' | '\\' => {
                let idx = (i / 2) % colors.len();
                result.push_str(&format!("{}{}{}", colors[idx], ch, reset));
            }
            '-' => result.push_str(&format!("{}{}{}", "\x1b[90m", ch, reset)),
            _ => result.push(ch),
        }
    }
    result
}
