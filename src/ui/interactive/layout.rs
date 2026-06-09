//! Shared vertical layouts for interactive screens.
//!
//! Keeping the zone math in one place means every prompt lines its header,
//! body and help bar up identically.

use ratatui::layout::{Constraint, Layout, Rect};

/// `[header=1][body=fill][help=1]` — the minimal prompt frame.
#[allow(dead_code)]
pub fn three_zone(area: Rect) -> [Rect; 3] {
    let c = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);
    [c[0], c[1], c[2]]
}

/// `[header=1][body=fill][paginator=1][help=1]` — for scrolling lists.
pub fn four_zone(area: Rect) -> [Rect; 4] {
    let c = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    [c[0], c[1], c[2], c[3]]
}

/// Split `area` into rows with the given constraints (thin wrapper for the
/// bespoke layouts used by `input` and `confirm`).
pub fn rows<const N: usize>(area: Rect, constraints: [Constraint; N]) -> Vec<Rect> {
    Layout::vertical(constraints).split(area).to_vec()
}
