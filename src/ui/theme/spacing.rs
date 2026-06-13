//! [`Spacing`] — layout tokens (indent, gaps, blank-line counts) driven by the
//! theme's `density` setting.

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
