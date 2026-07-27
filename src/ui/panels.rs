//! Reusable panel component for structured layouts.
//!
//! Provides bordered panels with titles, dividers, and content lines.
//! All output respects terminal width.

use super::print::{muted, text_bold};
use super::render::{indent, terminal_width};
use super::theme;

// ─── Panel ────────────────────────────────────────────────────────────────────

/// A bordered panel with optional title and content sections.
pub struct Panel {
    title: Option<String>,
    width: usize,
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
        let t = theme::current();
        let b = &t.borders;

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
        let t = theme::current();
        let b = &t.borders;
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
        let t = theme::current();
        let b = &t.borders;
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
        let t = theme::current();
        let b = &t.borders;
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
        let t = theme::current();
        let b = &t.borders;
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
