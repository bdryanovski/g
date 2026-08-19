//! Reusable panel components for structured layouts.
//!
//! Provides bordered panels with titles, dividers, and content lines.
//! All output respects terminal width.
//!
//! # Components
//!
//! - [`Panel`] — Generic bordered panel with muted borders (for structural content)
//! - [`InfoBox`] — PNPM-style notification box with colored borders (for messages)
//! - [`Diagram`] — ASCII art renderer inside panels

use super::print::{danger, muted, primary, success, text_bold, warning};
use super::render::{indent, paint, terminal_width};
use super::theme;
use ratatui::style::Color;

// ─── Panel ────────────────────────────────────────────────────────────────────

/// A bordered panel with optional title and content sections.
///
/// Always uses rounded corners (`╭╮╰╯`) for a modern look,
/// regardless of the theme's border_style setting.
pub struct Panel {
    title: Option<String>,
    width: usize,
}

/// Rounded border glyphs for Panel (always used regardless of theme).
struct RoundedBorders {
    horizontal: char,
    vertical: char,
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
    tee_left: char,
    tee_right: char,
}

impl RoundedBorders {
    fn new() -> Self {
        Self {
            horizontal: '─',
            vertical: '│',
            top_left: '╭',
            top_right: '╮',
            bottom_left: '╰',
            bottom_right: '╯',
            tee_left: '├',
            tee_right: '┤',
        }
    }
}

impl Panel {
    /// Create a new panel that spans the terminal width.
    pub fn new() -> Self {
        let width = terminal_width().saturating_sub(indent().len() + 4);
        Self { title: None, width }
    }

    /// Set the panel title (centered in header).
    pub fn title(mut self, title: &str) -> Self {
        self.title = Some(title.to_string());
        self
    }

    /// Print the panel header (top border with optional title).
    pub fn print_header(&self) {
        let b = RoundedBorders::new();

        if let Some(ref title) = self.title {
            // Centered title: total inner width = self.width
            // We want: ───Title─── where left + title + right = self.width
            let title_len = console::measure_text_width(title);
            let remaining = self.width.saturating_sub(title_len);
            let padding_left = remaining / 2;
            let padding_right = remaining - padding_left;

            println!(
                "{}{}{}{}{}{}",
                indent(),
                muted(&b.top_left.to_string()),
                muted(&b.horizontal.to_string().repeat(padding_left)),
                text_bold(title),
                muted(&b.horizontal.to_string().repeat(padding_right)),
                muted(&b.top_right.to_string()),
            );
        } else {
            println!(
                "{}{}{}{}",
                indent(),
                muted(&b.top_left.to_string()),
                muted(&b.horizontal.to_string().repeat(self.width)),
                muted(&b.top_right.to_string()),
            );
        }
    }

    /// Print a content line (with side borders).
    pub fn print_line(&self, content: &str) {
        let b = RoundedBorders::new();
        let content_width = console::measure_text_width(content);
        // Account for the single space after left border
        let padding = self.width.saturating_sub(content_width + 1);

        println!(
            "{}{} {}{}{}",
            indent(),
            muted(&b.vertical.to_string()),
            content,
            " ".repeat(padding),
            muted(&b.vertical.to_string()),
        );
    }

    /// Print an empty line inside the panel.
    pub fn print_empty(&self) {
        let b = RoundedBorders::new();
        println!(
            "{}{}{}{}",
            indent(),
            muted(&b.vertical.to_string()),
            " ".repeat(self.width),
            muted(&b.vertical.to_string()),
        );
    }

    /// Print a horizontal divider inside the panel.
    pub fn print_divider(&self) {
        let b = RoundedBorders::new();
        println!(
            "{}{}{}{}",
            indent(),
            muted(&b.tee_left.to_string()),
            muted(&b.horizontal.to_string().repeat(self.width)),
            muted(&b.tee_right.to_string()),
        );
    }

    /// Print the panel footer (bottom border).
    pub fn print_footer(&self) {
        let b = RoundedBorders::new();
        println!(
            "{}{}{}{}",
            indent(),
            muted(&b.bottom_left.to_string()),
            muted(&b.horizontal.to_string().repeat(self.width)),
            muted(&b.bottom_right.to_string()),
        );
    }

    /// Get the inner content width (for text wrapping).
    pub fn inner_width(&self) -> usize {
        self.width.saturating_sub(2)
    }
}

impl Default for Panel {
    fn default() -> Self {
        Self::new()
    }
}

// ─── InfoBox ──────────────────────────────────────────────────────────────────

/// The kind of info box — controls border and title color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxKind {
    /// Primary/info (cyan) — neutral information
    Info,
    /// Success (green) — operation completed successfully
    Success,
    /// Warning (yellow) — non-blocking issue or caution
    Warning,
    /// Danger (red) — error or blocking issue
    Danger,
}

impl BoxKind {
    /// Get the color for this box kind from the current theme.
    fn color(&self) -> Color {
        let palette = &theme::current().palette;
        match self {
            BoxKind::Info => palette.primary,
            BoxKind::Success => palette.success,
            BoxKind::Warning => palette.warning,
            BoxKind::Danger => palette.danger,
        }
    }

    /// Get the color painter function for this box kind.
    fn paint(&self, text: &str) -> String {
        match self {
            BoxKind::Info => primary(text),
            BoxKind::Success => success(text),
            BoxKind::Warning => warning(text),
            BoxKind::Danger => danger(text),
        }
    }
}

/// PNPM-style notification box with colored borders.
///
/// Always uses rounded corners (`╭╮╰╯`) regardless of theme border_style
/// for a consistent modern look.
///
/// # Example
///
/// ```text
/// ╭ Warning ──────────────────────────────────────────────────────────────────────╮
/// │                                                                               │
/// │   Branch is 5 commits behind 'main'. Run `g workflow sync`.                   │
/// │                                                                               │
/// ╰───────────────────────────────────────────────────────────────────────────────╯
/// ```
pub struct InfoBox {
    kind: BoxKind,
    title: String,
    lines: Vec<String>,
    width: usize,
}

impl InfoBox {
    /// Create a new info box (cyan borders).
    pub fn info(title: &str) -> Self {
        Self::new(BoxKind::Info, title)
    }

    /// Create a new success box (green borders).
    pub fn success(title: &str) -> Self {
        Self::new(BoxKind::Success, title)
    }

    /// Create a new warning box (yellow borders).
    pub fn warning(title: &str) -> Self {
        Self::new(BoxKind::Warning, title)
    }

    /// Create a new danger/error box (red borders).
    pub fn danger(title: &str) -> Self {
        Self::new(BoxKind::Danger, title)
    }

    fn new(kind: BoxKind, title: &str) -> Self {
        let width = terminal_width().saturating_sub(indent().len() + 4);
        Self {
            kind,
            title: title.to_string(),
            lines: Vec::new(),
            width,
        }
    }

    /// Add a content line to the box.
    pub fn line(mut self, text: &str) -> Self {
        self.lines.push(text.to_string());
        self
    }

    /// Add a blank line to the box.
    pub fn blank(mut self) -> Self {
        self.lines.push(String::new());
        self
    }

    /// Render the box to stdout.
    pub fn print(&self) {
        let color = self.kind.color();

        // Rounded border glyphs (always, regardless of theme)
        let h = '─';
        let v = '│';
        let tl = '╭';
        let tr = '╮';
        let bl = '╰';
        let br = '╯';

        // Header: ╭ Title ─────────────────────────────────────────────────────────╮
        let title_with_spaces = format!(" {} ", self.title);
        let title_len = console::measure_text_width(&title_with_spaces);
        let remaining = self.width.saturating_sub(title_len);

        println!(
            "{}{}{}{}{}",
            indent(),
            paint(&tl.to_string(), color),
            self.kind.paint(&title_with_spaces),
            paint(&h.to_string().repeat(remaining), color),
            paint(&tr.to_string(), color),
        );

        // Empty line after header
        self.print_inner_line("", color, v);

        // Content lines
        for line in &self.lines {
            if line.is_empty() {
                self.print_inner_line("", color, v);
            } else {
                self.print_inner_line(&format!("  {}", line), color, v);
            }
        }

        // Empty line before footer
        self.print_inner_line("", color, v);

        // Footer: ╰───────────────────────────────────────────────────────────────────╯
        println!(
            "{}{}{}{}",
            indent(),
            paint(&bl.to_string(), color),
            paint(&h.to_string().repeat(self.width), color),
            paint(&br.to_string(), color),
        );
    }

    fn print_inner_line(&self, content: &str, color: Color, v: char) {
        let content_width = console::measure_text_width(content);
        let padding = self.width.saturating_sub(content_width + 1);

        println!(
            "{}{} {}{}{}",
            indent(),
            paint(&v.to_string(), color),
            content,
            " ".repeat(padding),
            paint(&v.to_string(), color),
        );
    }
}

// ─── Diagram ──────────────────────────────────────────────────────────────────

/// A diagram renderer for ASCII art (workflow diagrams, etc).
pub struct Diagram;

impl Diagram {
    /// Print a diagram inside a panel.
    pub fn print_in_panel(panel: &Panel, content: &str) {
        panel.print_empty();
        for line in content.lines() {
            if line.is_empty() {
                panel.print_empty();
            } else {
                panel.print_line(line);
            }
        }
        panel.print_empty();
    }
}
