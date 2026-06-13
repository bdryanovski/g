//! Theme system — semantic palette, icons, spacing, borders and component
//! styles, all assembled into a single [`Theme`] read from `theme::current()`.
//!
//! # Folder layout
//!
//! ```text
//! theme/
//!   mod.rs       ← this file: the Theme struct + current()/init() globals
//!   palette.rs   ← Palette (semantic role colors) + Palette::base()
//!   icons.rs     ← Icons (Unicode / ASCII fallbacks)
//!   spacing.rs   ← Spacing (density-driven indent / gaps)
//!   borders.rs   ← BorderStyle preset enum + Borders glyph set
//!   styles.rs    ← StyleSpec + Styles (named component presets)
//!   file.rs      ← built-in TOMLs + custom file loader + parse_color
//! ```
//!
//! # Design
//!
//! - [`Palette`] holds `ratatui::style::Color` values mapped to semantic roles
//!   (`primary`, `success`, …) rather than raw color names.
//! - [`Icons`] holds the Unicode strings used for status indicators (with an
//!   ASCII fallback for dumb terminals).
//! - The global [`current()`] accessor uses a [`std::sync::OnceLock`] so the
//!   theme is initialised exactly once and every subsequent call is a
//!   zero-cost pointer dereference.

mod borders;
mod file;
mod icons;
mod palette;
mod spacing;
mod styles;

use std::sync::OnceLock;

// Re-export every public name so `crate::ui::theme::Palette` etc. continue to
// work unchanged for all call sites outside this folder.
pub use borders::{BorderStyle, Borders};
pub use file::{builtin_names, materialize_builtin_themes, themes_dir};
// `parse_color` is public surface for theme tooling but currently has no
// in-crate caller — suppress the dead-code lint without removing the export.
#[allow(unused_imports)]
pub use file::parse_color;
pub use icons::Icons;
pub use palette::Palette;
pub use spacing::Spacing;
pub use styles::{StyleSpec, Styles};

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
        let toml_src = file::embedded_theme_toml(name)?;
        match file::parse_theme_str(toml_src) {
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
        let raw =
            std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        file::parse_theme_str(&raw)
    }

    /// The default dark-terminal theme.
    ///
    /// Colors match the output that the previous `colored`-based helpers
    /// produced, so the visual appearance is unchanged after migration.
    pub fn default_dark() -> Self {
        use ratatui::style::Color;
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
        Self::from_palette(palette)
    }

    /// Light-terminal theme — darker colors on a white/light background.
    ///
    /// Swaps bright colors for their darker ANSI equivalents so they remain
    /// readable against light terminal backgrounds.
    pub fn default_light() -> Self {
        use ratatui::style::Color;
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
        Self::from_palette(palette)
    }
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
