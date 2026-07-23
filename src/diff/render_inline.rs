//! Inline ANSI renderer — non-TTY / `--raw` fallback path.
//!
//! Emits the same [`crate::diff::parse::FileDiff`] data as the ratatui TUI but
//! as a line-by-line ANSI stream to stdout.  Used when:
//! - stdout is not a TTY (piped to a file or another program),
//! - the user passes `--raw` or sets `[diff].tool = "raw"`,
//! - `--no-interactive` is active.
//!
//! Highlighting comes from [`crate::diff::highlight`]; layout is a single
//! column (the unified-diff `stack` layout).  Split/side-by-side are not
//! available in this path — they are properties of the ratatui TUI only.

use std::io::{self, Write};

use ratatui::style::{Color, Modifier, Style};

use super::highlight::{self, HighlightedLine};
use super::parse::{FileDiff, Status};
use crate::ui::theme;

// ─── Public API ──────────────────────────────────────────────────────────────

/// Render `files` to stdout as ANSI-coloured text.
///
/// This is the single entry point used by the `g diff` dispatcher when the
/// builtin path is selected with a non-TTY stdout (or `--raw`).  It is
/// synchronous and prints directly to [`io::Stdout`]; no ratatui alternate
/// screen is entered.
pub fn render(files: &[FileDiff]) {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let _ = render_to(&mut out, files);
}

/// Render `files` to an arbitrary writer.  Tested in isolation.
pub fn render_to<W: Write>(out: &mut W, files: &[FileDiff]) -> io::Result<()> {
    let highlighted = highlight::files(files);
    for (i, (file, lines)) in files.iter().zip(highlighted.iter()).enumerate() {
        if i > 0 {
            writeln!(out)?;
        }
        render_file_header(out, file)?;
        render_stat(out, file)?;
        for line in lines {
            render_line(out, line)?;
        }
    }
    Ok(())
}

// ─── Renderers ───────────────────────────────────────────────────────────────

/// Emit the `diff --git …` file-header banner with the path prominent.
fn render_file_header<W: Write>(out: &mut W, file: &FileDiff) -> io::Result<()> {
    let t = theme::current();
    let banner = paint_bold(t.palette.accent, &format!("{} {}", status_glyph(file.status), file.path));
    if let Some(old) = &file.old_path {
        writeln!(out, "{} ← {}", banner, paint(t.palette.muted, old))?;
    } else {
        writeln!(out, "{}", banner)?;
    }
    Ok(())
}

/// One-line summary ("3 additions, 1 deletion in N hunks") — only when nonzero.
fn render_stat<W: Write>(out: &mut W, file: &FileDiff) -> io::Result<()> {
    if file.hunks.is_empty() {
        return Ok(());
    }
    let stat = file.stat();
    if stat.added == 0 && stat.deleted == 0 {
        return Ok(());
    }
    let t = theme::current();
    writeln!(
        out,
        "  {} {} {}",
        paint(t.palette.success, &stat.added.to_string()),
        paint(t.palette.danger, &stat.deleted.to_string()),
        paint_dim(t.palette.muted, &format!("in {} hunk(s)", file.hunks.len())),
    )
}

/// Render one highlighted line, prefixed with its marker colour + line numbers.
fn render_line<W: Write>(out: &mut W, line: &HighlightedLine) -> io::Result<()> {
    let t = theme::current();
    let marker_color = marker_color(line.marker);

    // Line-number gutter (space-4-padded on each side that exists).
    let old_no = line
        .old_no
        .map(|n| format!("{:>4}", n))
        .unwrap_or_else(|| "    ".to_string());
    let new_no = line
        .new_no
        .map(|n| format!("{:>4}", n))
        .unwrap_or_else(|| "    ".to_string());
    write!(
        out,
        "{}{} {} ",
        paint_dim(t.palette.muted, &old_no),
        paint_dim(t.palette.muted, &new_no),
        paint_bold(marker_color, &line.marker.to_string()),
    )?;
    for (style, text) in &line.spans {
        write_style(out, *style, text)?;
    }
    writeln!(out, "{}", reset())?;
    Ok(())
}

/// Resolve the marker colour from the active palette.
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

/// Translate the marker character into a one-character label.
fn status_glyph(s: Status) -> &'static str {
    match s {
        Status::Added => "+ added",
        Status::Modified => "~ modified",
        Status::Deleted => "- deleted",
        Status::Renamed => "→ renamed",
        Status::Copied => "⇉ copied",
        Status::ModeChange => "# mode",
        Status::Binary => "◽ binary",
        Status::TypeChange => "↦ typechange",
    }
}

// ─── ANSI primitives ─────────────────────────────────────────────────────────

/// Emit `text` in the given foreground colour, bold.
fn paint_bold(color: Color, text: &str) -> String {
    use crossterm::style::Stylize;
    text.with(ct(color)).bold().to_string()
}

/// Emit `text` in the given foreground colour.
fn paint(color: Color, text: &str) -> String {
    use crossterm::style::Stylize;
    text.with(ct(color)).to_string()
}

/// Emit `text` in the given foreground colour, dim.
fn paint_dim(color: Color, text: &str) -> String {
    use crossterm::style::{Attribute, Stylize};
    text.with(ct(color)).attribute(Attribute::Dim).to_string()
}

/// Write a single styled span — translated from a ratatui [`Style`].
fn write_style<W: Write>(out: &mut W, style: Style, text: &str) -> io::Result<()> {
    use crossterm::style::{Attribute as A, Stylize};
    let mut s = text.with(crossterm::style::Color::Reset);
    if let Some(fg) = style.fg {
        s = s.with(ct(fg));
    }
    let mods = style.add_modifier;
    if mods.contains(Modifier::BOLD) {
        s = s.attribute(A::Bold);
    }
    if mods.contains(Modifier::ITALIC) {
        s = s.attribute(A::Italic);
    }
    if mods.contains(Modifier::UNDERLINED) {
        s = s.attribute(A::Underlined);
    }
    if mods.contains(Modifier::DIM) {
        s = s.attribute(A::Dim);
    }
    write!(out, "{}", s)
}

/// ANSI reset sequence.
fn reset() -> &'static str {
    "\x1b[0m"
}

/// Convert a ratatui [`Color`] to a `crossterm` colour.
fn ct(color: Color) -> crossterm::style::Color {
    match color {
        Color::Reset => crossterm::style::Color::Reset,
        Color::Black => crossterm::style::Color::Black,
        Color::Red => crossterm::style::Color::DarkRed,
        Color::Green => crossterm::style::Color::DarkGreen,
        Color::Yellow => crossterm::style::Color::DarkYellow,
        Color::Blue => crossterm::style::Color::DarkBlue,
        Color::Magenta => crossterm::style::Color::DarkMagenta,
        Color::Cyan => crossterm::style::Color::DarkCyan,
        Color::Gray => crossterm::style::Color::Grey,
        Color::DarkGray => crossterm::style::Color::DarkGrey,
        Color::LightRed => crossterm::style::Color::Red,
        Color::LightGreen => crossterm::style::Color::Green,
        Color::LightYellow => crossterm::style::Color::Yellow,
        Color::LightBlue => crossterm::style::Color::Blue,
        Color::LightMagenta => crossterm::style::Color::Magenta,
        Color::LightCyan => crossterm::style::Color::Cyan,
        Color::White => crossterm::style::Color::White,
        Color::Rgb(r, g, b) => crossterm::style::Color::Rgb { r, g, b },
        Color::Indexed(i) => crossterm::style::Color::AnsiValue(i),
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::parse;

    fn sample_diff() -> Vec<FileDiff> {
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
        parse::parse(raw)
    }

    #[test]
    fn render_to_produces_nonempty_output() {
        // Force a theme initialised before writing.
        let _ = theme::current();
        let files = sample_diff();
        let mut buf = Vec::new();
        render_to(&mut buf, &files).expect("render");
        let out = String::from_utf8_lossy(&buf);
        // Strip ANSI escape sequences so we can assert on the *text* content
        // (highlighter inserts colour codes between tokens e.g. "fn" + ANSI + "main").
        let stripped = strip_ansi(&out);
        assert!(stripped.contains("modified"), "header present: {stripped}");
        assert!(stripped.contains("fn main()"), "body present: {stripped}");
        assert!(stripped.contains('-') || stripped.contains("−"), "has a deletion: {stripped}");
    }

    /// Strip ANSI SGR escape sequences (`\x1b[…m`) for test comparisons.
    fn strip_ansi(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        let mut chars = input.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                // Consume `[` … letter sequence.
                if chars.next() == Some('[') {
                    for c in chars.by_ref() {
                        if c.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }
}