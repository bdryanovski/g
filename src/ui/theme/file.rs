//! Theme file I/O: built-ins embedded at compile time, custom files loaded
//! from disk, and the color-string parser.
//!
//! Everything in this module deals with *strings* — TOML on disk and color
//! literals like `"#ff8800"` or `"brightcyan"` — and turns them into the
//! typed theme structures defined in the sibling modules.

use ratatui::style::Color;
use serde::Deserialize;

use super::{BorderStyle, Icons, Palette, Spacing, Styles, Theme};

// ─── Embedded built-in theme files ───────────────────────────────────────────

/// Built-in themes shipped as TOML under `themes/` and embedded at compile
/// time, paired with their canonical names.  This is the single source of
/// truth used both for resolution and for materialising editable copies into
/// the user themes directory.
const BUILTIN_THEMES: &[(&str, &str)] = &[
    ("dark", include_str!("../../../themes/dark.toml")),
    ("light", include_str!("../../../themes/light.toml")),
    ("dracula", include_str!("../../../themes/dracula.toml")),
    ("nord", include_str!("../../../themes/nord.toml")),
    ("gruvbox", include_str!("../../../themes/gruvbox.toml")),
    (
        "solarized-dark",
        include_str!("../../../themes/solarized-dark.toml"),
    ),
    (
        "monochrome",
        include_str!("../../../themes/monochrome.toml"),
    ),
];

/// Return the embedded TOML source for built-in `name` (case-insensitive,
/// accepting a few aliases), or `None` if there is no such built-in.
pub(super) fn embedded_theme_toml(name: &str) -> Option<&'static str> {
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
pub(super) fn parse_theme_str(raw: &str) -> Result<Theme, String> {
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
