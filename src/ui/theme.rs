//! Theme system — semantic color palette and icon set.
//!
//! All colors in the application are derived from the active [`Theme`].
//! Command modules never reference raw colors; they call helpers in
//! [`crate::ui`] which read from `theme::current()` at render time.
//!
//! # Design
//!
//! - [`Palette`] holds `ratatui::style::Color` values mapped to semantic roles
//!   (`primary`, `success`, …) rather than raw color names.  Swapping a theme
//!   later only requires changing this struct.
//! - [`Icons`] holds the Unicode strings used for status indicators.  Replacing
//!   them with plain ASCII variants would support dumb terminals without
//!   touching any rendering code.
//! - The global [`current()`] accessor uses a [`std::sync::OnceLock`] so the
//!   theme is initialised exactly once and every subsequent call is a
//!   zero-cost pointer dereference.

use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;
use std::sync::OnceLock;

// ─── Palette ──────────────────────────────────────────────────────────────────

/// Semantic color assignments for the active theme.
///
/// Each field maps a UI role to a `ratatui::style::Color`.  Internal rendering
/// helpers convert these to crossterm colors at output time.
pub struct Palette {
    /// Primary accent — info icons, spinner, active branch names.
    pub primary: Color,
    /// Success / added — checkmark icon, added lines, current branch.
    pub success: Color,
    /// Warning — warning icon, commit hashes, staged changes.
    pub warning: Color,
    /// Danger / error — error icon, deleted lines, remote refs.
    pub danger: Color,
    /// Muted — dates, dim secondary text, graph lines, dividers.
    pub muted: Color,
    /// Body text — general readable content.
    pub text: Color,
    /// Accent — refactor type prefix, special refs, tags.
    pub accent: Color,
    /// Divider fill — slash characters in fieldset separators.
    pub divider: Color,

    // ── Conventional commit prefix colors ──────────────────────────────────
    /// `feat:` prefix color.
    pub cc_feat: Color,
    /// `fix:` prefix color.
    pub cc_fix: Color,
    /// `docs:` prefix color.
    pub cc_docs: Color,
    /// `refactor:` prefix color.
    pub cc_refactor: Color,
    /// `perf:` prefix color.
    pub cc_perf: Color,
    /// `test:` prefix color.
    pub cc_test: Color,
    /// `chore:` / `build:` / `ci:` prefix color.
    pub cc_chore: Color,
    /// `revert:` prefix color.
    pub cc_revert: Color,
}

// ─── Icons ────────────────────────────────────────────────────────────────────

/// Unicode icon set used throughout the UI.
///
/// Keeping icons in one struct makes it easy to swap to plain-ASCII
/// alternatives for environments that cannot render Unicode.
pub struct Icons {
    /// Info message indicator.
    pub info: &'static str,
    /// Success / completion indicator.
    pub success: &'static str,
    /// Warning indicator.
    pub warning: &'static str,
    /// Error indicator.
    pub error: &'static str,
    /// Tip / hint indicator (reserved for future keybinding help bar).
    #[allow(dead_code)]
    pub tip: &'static str,
    /// Currently-checked-out branch marker.
    pub current: &'static str,
    /// Other (non-current) branch marker.
    pub other: &'static str,
    /// Commits-ahead arrow.
    pub ahead: &'static str,
    /// Commits-behind arrow.
    pub behind: &'static str,
    /// Added-file status icon.
    pub added: &'static str,
    /// Modified-file status icon.
    pub modified: &'static str,
    /// Deleted-file status icon.
    pub deleted: &'static str,
    /// Renamed-file status icon.
    pub renamed: &'static str,
}

// ─── Spacing ──────────────────────────────────────────────────────────────────

/// Layout spacing tokens.
///
/// Every gap, indent and inter-section blank line is derived from here, so a
/// single `density` setting can make the entire UI compact or roomy without
/// touching call sites.
#[allow(dead_code)] // several tokens are forward-looking, wired in incrementally
pub struct Spacing {
    /// Left margin applied to (almost) every output line.
    pub indent: &'static str,
    /// Horizontal gap inserted between table columns.
    pub col_gap: &'static str,
    /// Gap between a key and its value in key/value listings.
    pub label_gap: &'static str,
    /// Padding placed on each side of a fieldset / section title.
    pub title_pad: &'static str,
    /// Number of blank lines printed between top-level sections.
    pub section_gap: u8,
    /// Number of blank lines printed between items in a list.
    pub item_gap: u8,
}

// ─── Borders / glyphs ───────────────────────────────────────────────────────

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

// ─── Component style presets ────────────────────────────────────────────────

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

// ─── Theme ────────────────────────────────────────────────────────────────────

/// The complete UI theme: colors, icons, spacing, borders and component styles.
///
/// Constructed once via [`Theme::from_config`] and stored in a global
/// [`OnceLock`].  All rendering helpers read from [`current()`].
pub struct Theme {
    /// Color palette.
    pub palette: Palette,
    /// Icon set.
    pub icons: Icons,
    /// Layout spacing tokens.
    pub spacing: Spacing,
    /// Box-drawing / connector glyph set.
    pub borders: Borders,
    /// Reusable component style presets.
    pub styles: Styles,
}

impl Theme {
    /// Construct a theme from config values.
    ///
    /// - `mode` — the `theme` name (built-in, custom name, or path). The
    ///   resolved theme already carries its own border style and density, set
    ///   in its theme file.
    /// - `border_override` / `density_override` — optional `[ui]` overrides.
    ///   When `Some`, they take precedence over the theme's own choice; when
    ///   `None`, the theme decides.
    ///
    /// Any unrecognised override falls back to the default for that dimension,
    /// so a partially-specified config is always valid.
    pub fn from_config(
        mode: &str,
        border_override: Option<&str>,
        density_override: Option<&str>,
    ) -> Self {
        let mut theme = Self::resolve(mode).unwrap_or_else(Self::default_dark);
        if let Some(b) = border_override {
            theme.borders = BorderStyle::from_config(b).glyphs();
        }
        if let Some(d) = density_override {
            theme.spacing = Spacing::for_density(d);
        }
        theme
    }

    /// Resolve a `theme` config value to a concrete [`Theme`].
    ///
    /// Resolution order (first match wins):
    /// 1. An explicit path (`theme = "/path/to/x.toml"`).
    /// 2. A user file in the themes directory (`~/.config/g/themes/<name>.toml`).
    ///    These are the *editable* copies of the built-ins, written on first
    ///    run, so user edits take effect without recompiling.
    /// 3. The TOML built-in embedded in the binary (safety net if the user file
    ///    was deleted).
    ///
    /// Returns `None` when nothing matches, so the caller can fall back to a
    /// default.
    fn resolve(name: &str) -> Option<Self> {
        let name = name.trim();

        // 1. Explicit path.
        if name.ends_with(".toml") || name.contains('/') {
            return match Self::load_path(std::path::Path::new(name)) {
                Ok(t) => Some(t),
                Err(e) => {
                    eprintln!("warning: could not load theme '{}': {}", name, e);
                    None
                }
            };
        }

        // 2. Editable user file in the themes directory.
        if let Some(path) = themes_dir().map(|d| d.join(format!("{}.toml", name))) {
            if path.exists() {
                match Self::load_path(&path) {
                    Ok(t) => return Some(t),
                    Err(e) => {
                        eprintln!("warning: could not load theme '{}': {}", name, e);
                        return None;
                    }
                }
            }
        }

        // 3. Embedded built-in.
        if let Some(t) = Self::builtin(name) {
            return Some(t);
        }

        eprintln!("warning: theme '{}' not found", name);
        None
    }

    /// Return the built-in theme for `name`, parsed from the TOML shipped inside
    /// the binary, or `None` if there is no shipped theme with that name.
    ///
    /// Built-ins are defined entirely in `themes/*.toml` (embedded via
    /// `include_str!`), so they can be edited without touching Rust.  If an
    /// embedded file ever fails to parse, this falls back to the hard-coded
    /// [`Theme::default_dark`] / [`Theme::default_light`] safety net.
    pub fn builtin(name: &str) -> Option<Self> {
        let toml_src = embedded_theme_toml(name)?;
        match parse_theme_str(toml_src) {
            Ok(t) => Some(t),
            Err(e) => {
                eprintln!("warning: built-in theme '{}' is invalid: {}", name, e);
                Some(match name.trim().to_lowercase().as_str() {
                    "light" => Self::default_light(),
                    _ => Self::default_dark(),
                })
            }
        }
    }

    /// Build a complete theme from a [`Palette`], using the default Unicode
    /// icons, `normal` density and `sharp` borders.  Component styles are
    /// derived from the palette via [`Styles::from_palette`].
    pub fn from_palette(palette: Palette) -> Self {
        let styles = Styles::from_palette(&palette);
        Self {
            palette,
            icons: Icons::unicode(),
            spacing: Spacing::for_density("normal"),
            borders: BorderStyle::Sharp.glyphs(),
            styles,
        }
    }

    /// Load and parse a theme TOML file from `path`.
    fn load_path(path: &std::path::Path) -> Result<Self, String> {
        if !path.exists() {
            return Err(format!("{} not found", path.display()));
        }
        let raw = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        parse_theme_str(&raw)
    }

    /// The default dark-terminal theme.
    ///
    /// Colors match the output that the previous `colored`-based helpers
    /// produced, so the visual appearance is unchanged after migration.
    pub fn default_dark() -> Self {
        let palette = Palette {
            primary: Color::Cyan,
            success: Color::Green,
            warning: Color::Yellow,
            danger: Color::Red,
            muted: Color::DarkGray,
            text: Color::White,
            accent: Color::Magenta,
            divider: Color::DarkGray,
            cc_feat: Color::Green,
            cc_fix: Color::Red,
            cc_docs: Color::Blue,
            cc_refactor: Color::Magenta,
            cc_perf: Color::Yellow,
            cc_test: Color::Cyan,
            cc_chore: Color::DarkGray,
            cc_revert: Color::Red,
        };
        let styles = Styles::from_palette(&palette);
        Self {
            palette,
            icons: Icons::unicode(),
            spacing: Spacing::for_density("normal"),
            borders: BorderStyle::Sharp.glyphs(),
            styles,
        }
    }

    /// Light-terminal theme — darker colors on a white/light background.
    ///
    /// Swaps bright colors for their darker ANSI equivalents so they remain
    /// readable against light terminal backgrounds.
    pub fn default_light() -> Self {
        let palette = Palette {
            primary: Color::Blue, // visible blue on white
            success: Color::Green,
            warning: Color::Yellow,
            danger: Color::Red,
            muted: Color::DarkGray,
            text: Color::Black, // black text on white
            accent: Color::Magenta,
            divider: Color::DarkGray,
            cc_feat: Color::Green,
            cc_fix: Color::Red,
            cc_docs: Color::Blue,
            cc_refactor: Color::Magenta,
            cc_perf: Color::Yellow,
            cc_test: Color::Cyan,
            cc_chore: Color::DarkGray,
            cc_revert: Color::Red,
        };
        let styles = Styles::from_palette(&palette);
        Self {
            palette,
            icons: Icons::unicode(),
            spacing: Spacing::for_density("normal"),
            borders: BorderStyle::Sharp.glyphs(),
            styles,
        }
    }
}

impl Icons {
    /// The default Unicode icon set.
    pub fn unicode() -> Self {
        Self {
            info: "ℹ",
            success: "✓",
            warning: "⚠",
            error: "✗",
            tip: "▶",
            current: "◉",
            other: "◯",
            ahead: "↑",
            behind: "↓",
            added: "✚",
            modified: "✎",
            deleted: "✖",
            renamed: "➜",
        }
    }

    /// A pure-ASCII icon set for terminals that cannot render Unicode.
    pub fn ascii() -> Self {
        Self {
            info: "i",
            success: "+",
            warning: "!",
            error: "x",
            tip: ">",
            current: "*",
            other: "o",
            ahead: "^",
            behind: "v",
            added: "+",
            modified: "~",
            deleted: "-",
            renamed: ">",
        }
    }
}

impl Spacing {
    /// Build the spacing tokens for a named density preset.
    ///
    /// - `"compact"` — single-space indent, no inter-section blanks.
    /// - `"relaxed"` — wide indent and gaps, extra breathing room.
    /// - anything else — the balanced `"normal"` default.
    pub fn for_density(name: &str) -> Self {
        match name.trim().to_lowercase().as_str() {
            "compact" | "tight" => Self {
                indent: " ",
                col_gap: " ",
                label_gap: " ",
                title_pad: " ",
                section_gap: 0,
                item_gap: 0,
            },
            "relaxed" | "comfortable" | "spacious" => Self {
                indent: "    ",
                col_gap: "   ",
                label_gap: "  ",
                title_pad: "  ",
                section_gap: 2,
                item_gap: 1,
            },
            _ => Self {
                indent: "  ",
                col_gap: "  ",
                label_gap: " ",
                title_pad: "  ",
                section_gap: 1,
                item_gap: 0,
            },
        }
    }
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

// ─── Base palette ────────────────────────────────────────────────────────────

impl Palette {
    /// The neutral base palette used as the starting point when a theme file
    /// does not set `extends`.  Themes that specify every color override this
    /// completely; partial themes inherit the unspecified roles from here.
    ///
    /// Kept in Rust (rather than TOML) so there is always a valid palette even
    /// before any file is read.  Mirrors the `dark` built-in.
    pub fn base() -> Self {
        Self {
            primary: Color::Cyan,
            success: Color::Green,
            warning: Color::Yellow,
            danger: Color::Red,
            muted: Color::DarkGray,
            text: Color::White,
            accent: Color::Magenta,
            divider: Color::DarkGray,
            cc_feat: Color::Green,
            cc_fix: Color::Red,
            cc_docs: Color::Blue,
            cc_refactor: Color::Magenta,
            cc_perf: Color::Yellow,
            cc_test: Color::Cyan,
            cc_chore: Color::DarkGray,
            cc_revert: Color::Red,
        }
    }
}

// ─── Embedded built-in theme files ───────────────────────────────────────────

/// Built-in themes shipped as TOML under `themes/` and embedded at compile
/// time, paired with their canonical names.  This is the single source of
/// truth used both for resolution and for materialising editable copies into
/// the user themes directory.
const BUILTIN_THEMES: &[(&str, &str)] = &[
    ("dark", include_str!("../../themes/dark.toml")),
    ("light", include_str!("../../themes/light.toml")),
    ("dracula", include_str!("../../themes/dracula.toml")),
    ("nord", include_str!("../../themes/nord.toml")),
    ("gruvbox", include_str!("../../themes/gruvbox.toml")),
    ("solarized-dark", include_str!("../../themes/solarized-dark.toml")),
    ("monochrome", include_str!("../../themes/monochrome.toml")),
];

/// Return the embedded TOML source for built-in `name` (case-insensitive,
/// accepting a few aliases), or `None` if there is no such built-in.
fn embedded_theme_toml(name: &str) -> Option<&'static str> {
    let lower = name.trim().to_lowercase();
    let key = match lower.as_str() {
        "gruvbox-dark" => "gruvbox",
        "solarized" => "solarized-dark",
        "mono" => "monochrome",
        other => other,
    };
    BUILTIN_THEMES
        .iter()
        .find(|(n, _)| *n == key)
        .map(|(_, src)| *src)
}

/// Parse a theme TOML string into a complete [`Theme`].
fn parse_theme_str(raw: &str) -> Result<Theme, String> {
    let spec: ThemeFile = toml::from_str(raw).map_err(|e| e.to_string())?;
    spec.into_theme()
}

/// Write the editable copies of every built-in theme into the user themes
/// directory (`~/.config/g/themes`).  Existing files are left untouched, so a
/// user's edits are never overwritten.  Called once at startup.
pub fn materialize_builtin_themes() -> std::io::Result<()> {
    let Some(dir) = themes_dir() else {
        return Ok(());
    };
    std::fs::create_dir_all(&dir)?;
    for (name, src) in BUILTIN_THEMES {
        let path = dir.join(format!("{}.toml", name));
        if !path.exists() {
            std::fs::write(&path, src)?;
        }
    }
    Ok(())
}

/// The names of every shipped built-in theme, for listings and help text.
pub fn builtin_names() -> &'static [&'static str] {
    &[
        "dark",
        "light",
        "dracula",
        "nord",
        "gruvbox",
        "solarized-dark",
        "monochrome",
    ]
}

/// Return the user themes directory (`~/.config/g/themes`), if resolvable.
pub fn themes_dir() -> Option<std::path::PathBuf> {
    crate::config::config_dir().ok().map(|d| d.join("themes"))
}

// ─── Custom theme files ───────────────────────────────────────────────────────

/// On-disk schema for a custom theme TOML file.
///
/// Every field is optional so a theme can override as little or as much as it
/// likes.  Colors that are not set are inherited from the `extends` base theme
/// (defaulting to `dark`).
#[derive(Debug, Deserialize)]
struct ThemeFile {
    /// Display name (informational only).
    #[allow(dead_code)]
    name: Option<String>,
    /// Built-in theme to start from (`"dark"`, `"nord"`, …).  Defaults to dark.
    extends: Option<String>,
    /// Border preset override.
    border_style: Option<String>,
    /// Density override.
    density: Option<String>,
    /// Use the ASCII icon set instead of Unicode.
    ascii_icons: Option<bool>,
    /// Per-role color overrides.
    #[serde(default)]
    palette: PaletteSpec,
}

/// Optional per-role color overrides parsed from a theme file.  Each value is a
/// color string accepted by [`parse_color`] (a name, `#RRGGBB`, or `0`–`255`).
#[derive(Debug, Default, Deserialize)]
struct PaletteSpec {
    primary: Option<String>,
    success: Option<String>,
    warning: Option<String>,
    danger: Option<String>,
    muted: Option<String>,
    text: Option<String>,
    accent: Option<String>,
    divider: Option<String>,
    cc_feat: Option<String>,
    cc_fix: Option<String>,
    cc_docs: Option<String>,
    cc_refactor: Option<String>,
    cc_perf: Option<String>,
    cc_test: Option<String>,
    cc_chore: Option<String>,
    cc_revert: Option<String>,
}

impl ThemeFile {
    /// Materialise the file spec into a complete [`Theme`], layering color
    /// overrides on top of the `extends` base palette.
    fn into_theme(self) -> Result<Theme, String> {
        // With no `extends`, start from the neutral base palette (kept in Rust)
        // — this avoids any recursion when parsing the built-in theme files
        // themselves, which do not set `extends`.
        let mut base = match self.extends.as_deref() {
            Some(name) => Theme::builtin(name)
                .ok_or_else(|| format!("unknown base theme in `extends`: {}", name))?,
            None => Theme::from_palette(Palette::base()),
        };

        // Apply color overrides.
        let p = &mut base.palette;
        let s = self.palette;
        set_color(&mut p.primary, &s.primary)?;
        set_color(&mut p.success, &s.success)?;
        set_color(&mut p.warning, &s.warning)?;
        set_color(&mut p.danger, &s.danger)?;
        set_color(&mut p.muted, &s.muted)?;
        set_color(&mut p.text, &s.text)?;
        set_color(&mut p.accent, &s.accent)?;
        set_color(&mut p.divider, &s.divider)?;
        set_color(&mut p.cc_feat, &s.cc_feat)?;
        set_color(&mut p.cc_fix, &s.cc_fix)?;
        set_color(&mut p.cc_docs, &s.cc_docs)?;
        set_color(&mut p.cc_refactor, &s.cc_refactor)?;
        set_color(&mut p.cc_perf, &s.cc_perf)?;
        set_color(&mut p.cc_test, &s.cc_test)?;
        set_color(&mut p.cc_chore, &s.cc_chore)?;
        set_color(&mut p.cc_revert, &s.cc_revert)?;

        // Re-derive component styles from the (possibly overridden) palette.
        base.styles = Styles::from_palette(&base.palette);

        if let Some(b) = &self.border_style {
            base.borders = BorderStyle::from_config(b).glyphs();
        }
        if let Some(d) = &self.density {
            base.spacing = Spacing::for_density(d);
        }
        if self.ascii_icons.unwrap_or(false) {
            base.icons = Icons::ascii();
        }
        Ok(base)
    }
}

/// Overwrite `target` with the parsed color when `spec` is `Some`.
fn set_color(target: &mut Color, spec: &Option<String>) -> Result<(), String> {
    if let Some(raw) = spec {
        *target = parse_color(raw)?;
    }
    Ok(())
}

/// Parse a color string into a [`Color`].
///
/// Accepted forms:
/// - hex: `#RGB` or `#RRGGBB`
/// - 256-color index: `0`–`255`
/// - ANSI names: `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`,
///   `gray`/`grey`, `darkgray`, `white`, and `bright*` / `light*` variants
pub fn parse_color(raw: &str) -> Result<Color, String> {
    let s = raw.trim();
    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Ok(idx) = s.parse::<u8>() {
        return Ok(Color::Indexed(idx));
    }
    let c = match s.to_lowercase().replace(['_', '-', ' '], "").as_str() {
        "black" => Color::Black,
        "red" | "darkred" => Color::Red,
        "green" | "darkgreen" => Color::Green,
        "yellow" | "darkyellow" => Color::Yellow,
        "blue" | "darkblue" => Color::Blue,
        "magenta" | "purple" | "darkmagenta" => Color::Magenta,
        "cyan" | "darkcyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "white" => Color::White,
        "brightred" | "lightred" => Color::LightRed,
        "brightgreen" | "lightgreen" => Color::LightGreen,
        "brightyellow" | "lightyellow" => Color::LightYellow,
        "brightblue" | "lightblue" => Color::LightBlue,
        "brightmagenta" | "lightmagenta" => Color::LightMagenta,
        "brightcyan" | "lightcyan" => Color::LightCyan,
        "reset" | "default" => Color::Reset,
        _ => return Err(format!("unrecognised color: '{}'", raw)),
    };
    Ok(c)
}

/// Parse a `RGB` (3-digit) or `RRGGBB` (6-digit) hex string into `Color::Rgb`.
fn parse_hex(hex: &str) -> Result<Color, String> {
    let expand = |s: &str| -> Option<(u8, u8, u8)> {
        match s.len() {
            3 => {
                let f = |c: char| u8::from_str_radix(&c.to_string().repeat(2), 16).ok();
                let mut it = s.chars();
                Some((f(it.next()?)?, f(it.next()?)?, f(it.next()?)?))
            }
            6 => Some((
                u8::from_str_radix(&s[0..2], 16).ok()?,
                u8::from_str_radix(&s[2..4], 16).ok()?,
                u8::from_str_radix(&s[4..6], 16).ok()?,
            )),
            _ => None,
        }
    };
    expand(hex)
        .map(|(r, g, b)| Color::Rgb(r, g, b))
        .ok_or_else(|| format!("invalid hex color: '#{}'", hex))
}

// ─── Global accessor ──────────────────────────────────────────────────────────

static THEME: OnceLock<Theme> = OnceLock::new();

/// Return a reference to the active theme.
///
/// On the first call, initialises the theme to [`Theme::default_dark`].
/// Call [`init`] before the first output statement if you want a
/// different theme.
pub fn current() -> &'static Theme {
    THEME.get_or_init(Theme::default_dark)
}

/// Set the global theme.
///
/// Must be called before any UI output.  Subsequent calls are ignored
/// (the `OnceLock` writes only once).
#[allow(dead_code)]
pub fn init(theme: Theme) {
    let _ = THEME.set(theme);
}
