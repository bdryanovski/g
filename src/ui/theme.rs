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

use ratatui::style::Color;
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

// ─── Theme ────────────────────────────────────────────────────────────────────

/// The complete UI theme: colors and icons.
///
/// Constructed once via [`Theme::default_dark`] (or a future `from_config`)
/// and stored in a global [`OnceLock`].  All rendering helpers read from
/// [`current()`].
pub struct Theme {
    /// Color palette.
    pub palette: Palette,
    /// Icon set.
    pub icons: Icons,
}

impl Theme {
    /// Construct a theme from a config string.
    ///
    /// Recognises `"dark"` (default) and `"light"`.  Any unrecognised value
    /// falls back to `default_dark()`.
    pub fn from_config(mode: &str) -> Self {
        match mode.trim().to_lowercase().as_str() {
            "light" => Self::default_light(),
            _ => Self::default_dark(),
        }
    }

    /// The default dark-terminal theme.
    ///
    /// Colors match the output that the previous `colored`-based helpers
    /// produced, so the visual appearance is unchanged after migration.
    pub fn default_dark() -> Self {
        Self {
            palette: Palette {
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
            },
            icons: Icons {
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
            },
        }
    }

    /// Light-terminal theme — darker colors on a white/light background.
    ///
    /// Swaps bright colors for their darker ANSI equivalents so they remain
    /// readable against light terminal backgrounds.
    pub fn default_light() -> Self {
        Self {
            palette: Palette {
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
            },
            icons: Icons {
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
            },
        }
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
