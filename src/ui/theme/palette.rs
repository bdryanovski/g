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

    // ── Syntax highlighting colors ─────────────────────────────────────────
    /// Keywords (`fn`, `let`, `if`, `match`, etc.).
    pub syntax_keyword: Color,
    /// Strings and string literals.
    pub syntax_string: Color,
    /// Comments.
    pub syntax_comment: Color,
    /// Function and method names.
    pub syntax_function: Color,
    /// Types, structs, enums, traits.
    pub syntax_type: Color,
    /// Numbers and numeric literals.
    pub syntax_number: Color,
    /// Operators (`+`, `-`, `=`, `::`, etc.).
    pub syntax_operator: Color,
    /// Variables and identifiers.
    pub syntax_variable: Color,
    /// Constants and statics.
    pub syntax_constant: Color,
    /// Attributes and annotations (`#[...]`).
    pub syntax_attribute: Color,

    // ── Diff-specific colors ───────────────────────────────────────────────
    /// Added line background (subtle).
    pub diff_add_bg: Color,
    /// Deleted line background (subtle).
    pub diff_del_bg: Color,
    /// Hunk header color.
    pub diff_hunk: Color,
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
            // Syntax highlighting (dark theme defaults)
            syntax_keyword: Color::Magenta,
            syntax_string: Color::Green,
            syntax_comment: Color::DarkGray,
            syntax_function: Color::Blue,
            syntax_type: Color::Yellow,
            syntax_number: Color::Cyan,
            syntax_operator: Color::White,
            syntax_variable: Color::White,
            syntax_constant: Color::Cyan,
            syntax_attribute: Color::Yellow,
            // Diff colors
            diff_add_bg: Color::Rgb(0, 40, 0),
            diff_del_bg: Color::Rgb(40, 0, 0),
            diff_hunk: Color::Cyan,
        }
    }
}
