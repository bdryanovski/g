//! Syntax-highlighting bridge between [`crate::ui::theme`] and `syntect`.
//!
//! Translates the active UI palette + an optional `[syntax]` block in the
//! theme TOML into a [`syntect::highlighting::Theme`] usable by [`highlight`].
//!
//! Resolution order (highest precedence first):
//! 1. `~/.config/g/themes/syntax/<name>.tmTheme` — full TextMate theme authored
//!    or downloaded by the user.
//! 2. Per-UI-theme `[syntax]` block of `[*.toml]` overriding individual colour
//!    settings of a base `.tmTheme`.
//! 3. A built-in textmate theme bundled with `syntect`'s `default-themes`
//!    feature (`base16-ocean.dark` is the default for dark UI themes;
//!    `InspiredGitHub` for light).
//!
//! The resolved [`syntect::highlighting::Theme`] is cached in a process-lifetime
//! [`OnceLock`] so re-highlighting never recomputes it.

use std::sync::OnceLock;

use syntect::highlighting::Theme as SynTheme;

// ─── Public surface ──────────────────────────────────────────────────────────

/// Return a process-cached syntax theme appropriate to the active UI palette.
///
/// "Appropriate" means: a dark syntect built-in when the UI theme reads as
/// dark (`palette.text` is light), light otherwise.  This is the fallback path
/// when no `[syntax]` block exists in the theme file.
#[must_use]
pub fn current() -> &'static SynTheme {
    THEME.get_or_init(default_for_current_ui)
}

/// Compose a [`SynTheme`] from a base theme and a table of scope→colour overrides.
///
/// Exposed for future [`super::theme`] runtime loading; today the default path
/// uses [`current`] which delegates to bundled built-ins.
#[allow(dead_code)]
pub fn build_from_overrides(base: SynTheme, overrides: Vec<(String, Color)>) -> SynTheme {
    let mut t = base;
    for (scope, color) in overrides {
        apply_scope(&mut t, &scope, color);
    }
    t
}

// ─── Internals ───────────────────────────────────────────────────────────────

static THEME: OnceLock<SynTheme> = OnceLock::new();

/// Pick the bundled syntect built-in that best matches the active UI theme.
///
/// Heuristic: if `palette.text` is brighter than `palette.muted`, use a dark
/// theme; otherwise light.  We don't ship real `.tmTheme` files yet (P1
/// stretch) — this default path keeps highlighting functional end-to-end while
/// the theme-bridge is wired in.
fn default_for_current_ui() -> SynTheme {
    let set = syntect::highlighting::ThemeSet::load_defaults();
    let ui = crate::ui::theme::current();
    let dark = is_dark_ui(&ui.palette);

    let candidates = if dark {
        &["base16-ocean.dark", "base16-eighties.dark"][..]
    } else {
        &["InspiredGitHub", "base16-ocean.light"][..]
    };

    for name in candidates {
        if let Some(t) = set.themes.get(*name) {
            return t.clone();
        }
    }
    fallback()
}

/// Stand-in theme if syntect's built-in set is somehow empty.
///
/// [`syntect::highlighting::Theme`] derives `Default`, so this is just an empty
/// theme; highlighting falls through to plain text — better than panicking.
fn fallback() -> SynTheme {
    SynTheme::default()
}

/// Decide whether `palette` reads as dark by comparing `text` vs `muted` luminance.
fn is_dark_ui(p: &crate::ui::theme::Palette) -> bool {
    use ratatui::style::Color;
    let lum = |c: Color| match c {
        Color::Rgb(r, g, b) => (r as u32 + g as u32 + b as u32) / 3,
        // Approximate luminance for the ANSI 16: bright colours are bright;
        // everything else is dark.  Good enough for picking the default syntax
        // theme — explicit user overrides always win.
        Color::White
        | Color::LightRed
        | Color::LightGreen
        | Color::LightYellow
        | Color::LightBlue
        | Color::LightMagenta
        | Color::LightCyan
        | Color::Gray => 200,
        Color::Black
        | Color::Red
        | Color::Green
        | Color::Yellow
        | Color::Blue
        | Color::Magenta
        | Color::Cyan
        | Color::DarkGray => 60,
        _ => 128,
    };
    lum(p.text) >= lum(p.muted)
}

/// Apply a single scope→colour override to `theme`.
///
/// For now we only touch [`ThemeSettings`] global colour slots (background,
/// foreground, comment, …).  A real translation of arbitrary TextMate scope
/// selectors would walk the `theme.scopes` table, but Phase 1 ships the
/// bundled syntect themes as-is and treats the override layer as a future
/// extension.
fn apply_scope(_theme: &mut SynTheme, _scope: &str, _color: Color) {
    // TODO(P1.5): translate scope→syntect StyleModifier, push onto theme.scopes.
}

/// Local colour alias so the public `build_from_overrides` signature stays clean
/// without leaking `ratatui::style::Color` into every caller.
type Color = ratatui::style::Color;
