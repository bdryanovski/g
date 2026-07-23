//! Full-screen ratatui TUI for the builtin diff viewer.
//!
//! This module is a thin re-export layer that delegates to
//! [`crate::ui::diff_tui`] where the actual implementation lives.
//! This preserves backward compatibility with existing call sites.
//!
//! See [`crate::ui::diff_tui`] for the full documentation.

pub use crate::ui::diff_tui::run;

// Re-export types that may be used by other modules in the future.
#[allow(unused_imports)]
pub use crate::ui::diff_tui::{AppState, DiffLayout};
