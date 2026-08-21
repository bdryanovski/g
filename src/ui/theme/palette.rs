//! [`Palette`] — semantic color assignments for the active theme.

use ratatui::style::Color;

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
