//! [`Icons`] — Unicode (or ASCII fallback) icon set used throughout the UI.

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
