//! Cross-section helpers: number formatting and the generic horizontal bar
//! chart used by `top_commands`, `repo_activity`, and `messages::top_authors`.

use crate::ui;
use crate::ui::indent;

/// Eight increasing block heights used for sparkline charts.
pub(super) const SPARK: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Render a horizontal bar chart to stdout.
///
/// Each bar uses `█` for filled cells and `░` for empty cells. The bar width
/// is scaled so the largest value fills `bar_width` columns.
pub(super) fn render_bar_chart(items: &[(String, i64)], bar_width: usize) {
    if items.is_empty() {
        ui::print_info("No data recorded yet.");
        return;
    }

    let max_count = items.iter().map(|(_, n)| *n).max().unwrap_or(1).max(1);
    let max_label = items
        .iter()
        .map(|(l, _)| console::measure_text_width(l))
        .max()
        .unwrap_or(0);

    for (label, count) in items {
        let filled = (*count as usize * bar_width) / max_count as usize;
        let bar = format!(
            "{}{}",
            ui::success(&"█".repeat(filled)),
            ui::muted(&"░".repeat(bar_width - filled))
        );
        let label_pad = " ".repeat(max_label - console::measure_text_width(label));
        println!(
            "{}{}{}  {}  {}",
            indent(),
            ui::paint_text(label),
            label_pad,
            bar,
            ui::muted(&count.to_string()),
        );
    }
}

/// Format a duration given in milliseconds into a human-readable string.
///
/// - < 1 000 ms   → `"N ms"`
/// - < 60 000 ms  → `"N.N s"`
/// - otherwise    → `"N min N s"`
pub(super) fn fmt_duration_ms(ms: f64) -> String {
    if ms < 1_000.0 {
        format!("{:.0} ms", ms)
    } else if ms < 60_000.0 {
        format!("{:.1} s", ms / 1_000.0)
    } else {
        let total_s = (ms / 1_000.0) as u64;
        format!("{} min {} s", total_s / 60, total_s % 60)
    }
}

/// Format a large integer with thousands separators: `1234567` → `"1,234,567"`.
pub(super) fn fmt_n(n: i64) -> String {
    if n < 0 {
        return format!("-{}", fmt_n(-n));
    }
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}
