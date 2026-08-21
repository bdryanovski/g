//! Terminal UI — all output, styling, spinners, tables, and git formatting.
//!
//! # Module layout
//!
//! ```text
//! ui/
//!   mod.rs        ← this file: public facade, re-exports everything
//!   theme.rs      ← Theme, Palette, Icons + global OnceLock accessor
//!   render.rs     ← ct_color, paint_*, Spinner, ProgressBar, terminal_width
//!   print.rs      ← Mode 1: print_info, print_success, semantic styling helpers
//!   widgets.rs    ← Mode 2: Fieldset, Table, CommitEntry, git color helpers
//!   interactive/  ← Mode 3: full-screen ratatui TUI kit (alternate screen)
//!     mod.rs        · the prompts: select, multi_select, input, confirm, fuzzy
//!     runtime.rs    · reusable enter/draw/key/restore event loop + TTY guards
//!     layout.rs     · shared vertical zone splits
//!     widgets.rs    · themed header, help, list, input line, paginator, scroll_list
//!   inline/       ← Mode 4: inline prompt kit (stays in scrollback)
//!     mod.rs        · the inline_* prompts
//!     runtime.rs    · raw-mode key loop + static header + TTY guard
//!     widgets.rs    · option/checkbox rows + in-place redraw
//! ```
//!
//! ## Building a new screen
//!
//! A full-screen prompt is just *state + a draw call + a key match*:
//!
//! ```ignore
//! interactive::runtime::run(
//!     0usize,                                   // state
//!     |f, &cursor| widgets::scroll_list(/* … */),   // draw each frame
//!     |cursor, key| match key { /* … */ Flow::Done(result) },
//! )
//! ```
//!
//! An inline prompt prints its header + body, then hands keys to `run_raw`.
//!
//! Command files import only `crate::ui` and call `ui::print_info(…)` etc.
//! They never reference the sub-modules directly, so the split is an
//! implementation detail that can evolve without touching call sites.
//!
//! # Design principles
//!
//! - **Single source of styling** — every color comes from `theme::current()`.
//! - **No raw `colored` / `indicatif`** — all output is crossterm-backed and
//!   theme-aware.
//! - **Commands own layout; ui owns style** — a command decides *what* to
//!   print; this module decides *how* it looks.

pub mod stage;
pub mod theme;

mod inline;
mod interactive;
mod print;
mod render;
mod widgets;

// ─── Re-exports ───────────────────────────────────────────────────────────────
//
// Everything a command file needs is accessible as `ui::*` without having to
// know which sub-module it lives in.

pub use render::{
    indent, is_inline_prompts, is_no_interactive, progress_bar, set_inline_prompts,
    set_no_interactive, spinner, spinner_error, spinner_success, terminal_width, ProgressBar,
};

pub use print::{
    accent, danger, danger_bold, dimmed, link, link_muted, link_primary_bold, muted, paint_text,
    primary, primary_bold, print_blank, print_error, print_indented, print_info,
    print_key_value_pairs, print_line, print_section, print_stack_banner, print_step,
    print_success, print_tip, print_warning, success, success_bold, text_bold, warning,
    warning_bold,
};

pub use widgets::{
    branch_marker, branch_name_colored, color_added, color_author, color_branch, color_date,
    color_deleted, color_hash, color_subject, colorize_graph, commit_subject_width,
    format_ahead_behind, print_fieldset, render_stat_bar, status_icon, CommitEntry, Table,
};

pub use interactive::{
    confirm, fuzzy_select, input, input_validated, multi_select, select, SelectOption,
};
