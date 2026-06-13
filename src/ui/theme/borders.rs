//! [`BorderStyle`] presets and the [`Borders`] glyph set they expand into.
//!
//! Selecting a `BorderStyle` swaps every box-drawing glyph the UI uses in
//! lock-step — sharp, rounded, heavy, double, or pure-ASCII.

/// The named border presets.  Selecting one swaps every box-drawing glyph in
/// the UI in lock-step, including the ASCII fallback for dumb terminals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    /// `┌─┐ │ └─┘` — square corners.
    Sharp,
    /// `╭─╮ │ ╰─╯` — rounded corners.
    Rounded,
    /// `┏━┓ ┃ ┗━┛` — heavy weight.
    Heavy,
    /// `╔═╗ ║ ╚═╝` — double lines.
    Double,
    /// `+-+ | +-+` — pure ASCII fallback.
    Ascii,
}

/// A complete set of box-drawing / connector glyphs.
///
/// Built once from a [`BorderStyle`] via [`BorderStyle::glyphs`]; rendering code
/// reads characters from here rather than hard-coding `"─"`, `"│"`, `"└"`, etc.
#[allow(dead_code)] // full glyph set exposed; some glyphs wired in incrementally
pub struct Borders {
    /// Which preset produced this set.
    pub style: BorderStyle,
    /// Horizontal line / rule fill (`─`).
    pub horizontal: char,
    /// Vertical line (`│`).
    pub vertical: char,
    /// Top-left corner (`┌`).
    pub top_left: char,
    /// Top-right corner (`┐`).
    pub top_right: char,
    /// Bottom-left corner (`└`).
    pub bottom_left: char,
    /// Bottom-right corner (`┘`).
    pub bottom_right: char,
    /// Left tee / tree branch (`├`).
    pub tee_left: char,
    /// Right tee (`┤`).
    pub tee_right: char,
    /// Tree "last child" connector (`└`).
    pub tree_last: char,
    /// Slash-divider fill used by fieldset section headers (`/`).
    pub divider_fill: char,
    /// In-line list bullet (`·`).
    pub bullet: char,
}

impl BorderStyle {
    /// Parse a config string (`"rounded"`, `"heavy"`, …) into a [`BorderStyle`].
    /// Unknown values fall back to [`BorderStyle::Sharp`].
    pub fn from_config(name: &str) -> Self {
        match name.trim().to_lowercase().as_str() {
            "rounded" | "round" => Self::Rounded,
            "heavy" | "bold" | "thick" => Self::Heavy,
            "double" => Self::Double,
            "ascii" | "plain" => Self::Ascii,
            _ => Self::Sharp,
        }
    }

    /// Expand the preset into a concrete [`Borders`] glyph set.
    pub fn glyphs(self) -> Borders {
        let (h, v, tl, tr, bl, br, el, er, last) = match self {
            Self::Sharp => ('─', '│', '┌', '┐', '└', '┘', '├', '┤', '└'),
            Self::Rounded => ('─', '│', '╭', '╮', '╰', '╯', '├', '┤', '╰'),
            Self::Heavy => ('━', '┃', '┏', '┓', '┗', '┛', '┣', '┫', '┗'),
            Self::Double => ('═', '║', '╔', '╗', '╚', '╝', '╠', '╣', '╚'),
            Self::Ascii => ('-', '|', '+', '+', '+', '+', '+', '+', '+'),
        };
        let bullet = if self == Self::Ascii { '*' } else { '·' };
        Borders {
            style: self,
            horizontal: h,
            vertical: v,
            top_left: tl,
            top_right: tr,
            bottom_left: bl,
            bottom_right: br,
            tee_left: el,
            tee_right: er,
            tree_last: last,
            divider_fill: '/',
            bullet,
        }
    }
}
