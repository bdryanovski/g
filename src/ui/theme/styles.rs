//! Reusable *component* styles: [`StyleSpec`] (color + modifiers) and the
//! named [`Styles`] set derived from a [`super::Palette`].
//!
//! Call sites ask for a role (`styles.section_title`) instead of choosing
//! "magenta + bold" directly, so re-skinning a component is a one-line change
//! in [`Styles::from_palette`].

use ratatui::style::{Color, Modifier, Style};

use super::Palette;

/// A reusable text style: a foreground color plus font modifiers.
///
/// `StyleSpec` is the building block for *components* — instead of a call site
/// choosing "magenta + bold", it asks the theme for `styles.section_title` and
/// renders that.  Re-theming a component is then a one-line change here.
#[derive(Debug, Clone, Copy)]
pub struct StyleSpec {
    /// Foreground color.
    pub color: Color,
    /// Apply the bold modifier.
    pub bold: bool,
    /// Apply the dim modifier.
    pub dim: bool,
    /// Apply the underline modifier.
    pub underline: bool,
}

impl StyleSpec {
    /// A plain (non-modified) style of the given color.
    pub const fn new(color: Color) -> Self {
        Self {
            color,
            bold: false,
            dim: false,
            underline: false,
        }
    }
    /// Builder: turn on bold.
    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
    /// Builder: turn on dim.
    pub const fn dim(mut self) -> Self {
        self.dim = true;
        self
    }
    /// Builder: turn on underline.
    #[allow(dead_code)]
    pub const fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// Convert to a ratatui [`Style`] for use with buffer-rendered widgets.
    #[allow(dead_code)]
    pub fn to_ratatui(self) -> Style {
        let mut s = Style::default().fg(self.color);
        if self.bold {
            s = s.add_modifier(Modifier::BOLD);
        }
        if self.dim {
            s = s.add_modifier(Modifier::DIM);
        }
        if self.underline {
            s = s.add_modifier(Modifier::UNDERLINED);
        }
        s
    }
}

/// Named, reusable component styles.
///
/// These describe *roles in composed components* (a section header, a banner,
/// a key/value pair, a badge) rather than raw palette colors, so the look of a
/// whole component can be re-skinned in one place.
#[allow(dead_code)] // reusable component styles; wired into call sites incrementally
pub struct Styles {
    /// Top-level section / fieldset title.
    pub section_title: StyleSpec,
    /// Horizontal rule / divider line.
    pub rule: StyleSpec,
    /// The key half of a key/value pair.
    pub key: StyleSpec,
    /// The value half of a key/value pair.
    pub value: StyleSpec,
    /// Emphasised banner text (e.g. the stack banner).
    pub banner: StyleSpec,
    /// Small inline badge / count chip.
    pub badge: StyleSpec,
    /// De-emphasised hint / help text.
    pub hint: StyleSpec,
}

impl Styles {
    /// Derive the reusable component styles from a color [`Palette`].
    ///
    /// Keeping this derivation in one place means a new palette automatically
    /// produces consistent component styling.
    pub fn from_palette(p: &Palette) -> Self {
        Self {
            section_title: StyleSpec::new(p.accent).bold(),
            rule: StyleSpec::new(p.muted),
            key: StyleSpec::new(p.muted),
            value: StyleSpec::new(p.text),
            banner: StyleSpec::new(p.primary).bold(),
            badge: StyleSpec::new(p.warning).bold(),
            hint: StyleSpec::new(p.muted).dim(),
        }
    }
}
