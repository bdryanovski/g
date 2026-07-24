//! `syntect`-based per-line syntax highlighter.
//!
//! Takes a [`super::parse::FileDiff`] (or individual hunk lines) and turns each
//! line's text into a list of `(ratatui::style::Style, String)` spans ready to
//! be painted by either [`super::render_inline`] or [`super::render_tui`].
//!
//! Design points:
//! - One [`syntect::easy::HighlightLines`] per file, keyed on the file extension
//!   via [`syntect::parsing::SyntaxSet::find_syntax_by_extension`].  Falls back
//!   to plain text when no grammar matches.
//! - The `+`/`-`/space marker is stripped *before* highlighting, then
//!   re-applied as a foreground-coloured span using the active UI palette
//!   (green for `+`, red for `-`, dim for context).
//! - Output is a [`HighlightedLine`]: a marker char, optional line numbers, and
//!   a `Vec<(Style, String)>` of styled spans.
//! - The [`Engine`] (syntax set + theme lookup) is initialised lazily once per
//!   process via a [`OnceLock`]; loading bundled grammars takes ~10 ms.

use std::sync::OnceLock;

use ratatui::style::{Color, Modifier, Style};

use super::parse::{FileDiff, Line};
use super::theme;

// ─── Public types ────────────────────────────────────────────────────────────

/// One fully-highlighted diff line, ready for rendering.
#[derive(Debug, Clone)]
pub struct HighlightedLine {
    /// `' '` / `'+'` / `'-'` — the unified-diff marker character.
    pub marker: char,
    /// Line number on the old side (left column), if any.
    pub old_no: Option<u32>,
    /// Line number on the new side (right column), if any.
    pub new_no: Option<u32>,
    /// Pre-rendered styled spans for the *content* (excluding the marker).
    pub spans: Vec<(Style, String)>,
    /// `true` for the `\ No newline at end of file` row.
    pub is_nonewline: bool,
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Acquire the process-lifetime [`Engine`]: loaded once, reused for every diff.
#[must_use]
pub fn engine() -> &'static Engine {
    ENGINE.get_or_init(Engine::init)
}

/// Highlight every line in `files`, returning one flat [`Vec<HighlightedLine>`] per file.
#[must_use]
pub fn files(files: &[FileDiff]) -> Vec<Vec<HighlightedLine>> {
    let engine = engine();
    files.iter().map(|f| engine.highlight_file(f)).collect()
}

// ─── Engine ──────────────────────────────────────────────────────────────────

/// Process-wide syntect engine — loaded once and reused for every diff.
///
/// The [`Engine`] holds a leaked `SyntaxSet` (bundled grammars) and provides
/// accessors that hand out `'static` references suitable for `HighlightLines`.
pub struct Engine {
    syntax_set: &'static syntect::parsing::SyntaxSet,
}

impl Engine {
    /// Initialise the engine by loading bundled grammars + themes.
    fn init() -> Self {
        // `default-syntaxes` ships a binary dump; `load_defaults_newlines`
        // returns a `SyntaxSet` configured to handle line-by-line parsing
        // (a hard requirement for `HighlightLines::highlight_line`).
        let syntax_set = syntect::parsing::SyntaxSet::load_defaults_newlines();
        Self {
            syntax_set: Box::leak(Box::new(syntax_set)),
        }
    }

    /// Resolve the [`syntect::parsing::SyntaxReference`] for `path` by extension.
    ///
    /// Falls back to plain-text when no grammar is registered for the
    /// extension or the path has no `.`-separated suffix.
    #[must_use]
    pub fn syntax_for_path(&self, path: &str) -> &'static syntect::parsing::SyntaxReference {
        let ext = path.rsplit('.').next().unwrap_or("");
        self.syntax_set
            .find_syntax_by_extension(ext)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text())
    }

    /// Highlight every line of `file` into a flat vector.
    ///
    /// A fresh [`syntect::easy::HighlightLines`] is created per hunk so the
    /// parser state doesn't bleed across unrelated hunks (matches `bat`'s
    /// behaviour for diff contexts).
    #[must_use]
    pub fn highlight_file(&self, file: &FileDiff) -> Vec<HighlightedLine> {
        let syntax = self.syntax_for_path(&file.path);
        let syntheme = theme::current();
        let ss = self.syntax_set;
        let mut out = Vec::new();
        for hunk in &file.hunks {
            let mut hl = syntect::easy::HighlightLines::new(syntax, syntheme);
            for line in &hunk.lines {
                highlight_line(&mut hl, ss, line, &mut out);
            }
        }
        out
    }
}

static ENGINE: OnceLock<Engine> = OnceLock::new();

// ─── Per-line highlighting ───────────────────────────────────────────────────

/// Highlight one [`Line`] and push the result onto `out`.
fn highlight_line(
    hl: &mut syntect::easy::HighlightLines<'static>,
    syntax_set: &syntect::parsing::SyntaxSet,
    line: &Line,
    out: &mut Vec<HighlightedLine>,
) {
    if matches!(line, Line::NoNewline) {
        let muted = Style::default().fg(crate::ui::theme::current().palette.muted);
        out.push(HighlightedLine {
            marker: '\\',
            old_no: None,
            new_no: None,
            spans: vec![(muted, " No newline at end of file".into())],
            is_nonewline: true,
        });
        return;
    }

    let text = line.text();
    let raw_spans = hl.highlight_line(text, syntax_set).unwrap_or_default();
    let spans: Vec<(Style, String)> = raw_spans
        .into_iter()
        .map(|(s, t)| (syntect_style_to_ratatui(s), t.to_string()))
        .collect();

    out.push(HighlightedLine {
        marker: line.marker(),
        old_no: line.old_no(),
        new_no: line.new_no(),
        spans,
        is_nonewline: false,
    });
}

/// Translate a `syntect` highlight style into a ratatui [`Style`].
fn syntect_style_to_ratatui(s: syntect::highlighting::Style) -> Style {
    let mut style = Style::default();
    let fg = s.foreground;
    style = style.fg(syntect_color_to_ratatui(fg));
    let mut mods = Modifier::empty();
    if s.font_style
        .contains(syntect::highlighting::FontStyle::BOLD)
    {
        mods |= Modifier::BOLD;
    }
    if s.font_style
        .contains(syntect::highlighting::FontStyle::ITALIC)
    {
        mods |= Modifier::ITALIC;
    }
    if s.font_style
        .contains(syntect::highlighting::FontStyle::UNDERLINE)
    {
        mods |= Modifier::UNDERLINED;
    }
    style.add_modifier(mods)
}

/// Translate a `syntect` RGBA colour into a ratatui [`Color`].
///
/// Fully-transparent colours (alpha 0) become [`Color::Reset`] so the renderer
/// can fall back to the active palette's body-text colour rather than painting
/// black-on-black.
fn syntect_color_to_ratatui(c: syntect::highlighting::Color) -> Color {
    if c.a == 0 {
        return Color::Reset;
    }
    Color::Rgb(c.r, c.g, c.b)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_is_cached() {
        let a = engine() as *const _;
        let b = engine() as *const _;
        assert_eq!(a, b, "engine() must return the same &'static Engine");
    }

    #[test]
    fn syntax_for_rust_path_resolves_rust_grammar() {
        let e = engine();
        let s = e.syntax_for_path("src/main.rs");
        // If the bundled defaults loaded correctly, the name should not be the
        // plain-text fallback ("Plain Text").  This also exercises the
        // grammar lookup on the live engine.
        assert_ne!(s.name, "Plain Text", "expected a real grammar for .rs");
    }

    #[test]
    fn syntax_for_unknown_extension_falls_back_to_plain_text() {
        let e = engine();
        let s = e.syntax_for_path("file.unknownext");
        assert_eq!(s.name, "Plain Text");
    }

    #[test]
    fn highlight_file_returns_lines_for_simple_rust_diff() {
        let raw = "\
diff --git a/x.rs b/x.rs
--- a/x.rs
+++ b/x.rs
@@ -1,2 +1,2 @@
 fn main() {
-    let x = 1;
+    let x = 2;
 }
";
        let files = super::super::parse::parse(raw);
        assert_eq!(files.len(), 1);
        let lines = engine().highlight_file(&files[0]);
        // 4 highlighted lines per the hunk's body (ctx, del, add, ctx).
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].marker, ' ');
        assert_eq!(lines[1].marker, '-');
        assert_eq!(lines[2].marker, '+');
        assert_eq!(lines[3].marker, ' ');
        // Each non-empty line has at least one span (could be more depending
        // on grammar tokenisation).
        assert!(!lines[0].spans.is_empty());
    }
}
