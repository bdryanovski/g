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
//!   interactive.rs← Mode 3: full-screen ratatui TUI (alternate screen)
//!   inline.rs     ← Mode 4: inline prompts (no alternate screen, stays in scrollback)
//! ```
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

// Some re-exported items are forward-looking public API (used in Phase 3 or
// by callers that reference them implicitly through type inference). Suppress
// the dead-code lint for those here rather than scattering allow-attributes.
#[allow(unused_imports)]
pub use render::{
    is_inline_prompts, is_no_interactive, paint, paint_bold, paint_bold_underline, paint_dim,
    paint_underline, progress_bar, set_inline_prompts, set_no_interactive, spinner, spinner_error,
    spinner_success, terminal_width, ProgressBar, Spinner, INDENT,
};

#[allow(unused_imports)]
pub use print::{
    accent, danger, danger_bold, dimmed, link, link_muted, link_primary_bold, muted, muted_bold,
    paint_text, primary, primary_bold, print_blank, print_error, print_indented, print_info,
    print_key_value_pairs, print_line, print_rule, print_section, print_stack_banner, print_step,
    print_success, print_tip, print_warning, success, success_bold, text_bold, warning,
    warning_bold,
};

#[allow(unused_imports)]
pub use widgets::{
    branch_marker, branch_name_colored, color_added, color_author, color_branch, color_date,
    color_deleted, color_hash, color_ref, color_subject, colorize_graph, commit_subject_width,
    format_ahead_behind, format_refs, print_fieldset, print_fieldset_count, print_stack_tree,
    render_stat_bar, status_icon, CommitEntry, Table,
};

#[allow(unused_imports)]
pub use interactive::{
    confirm, fuzzy_select, input, input_validated, multi_select, select, SelectOption,
};

#[allow(unused_imports)]
pub use inline::{
    inline_confirm, inline_fuzzy_select, inline_input, inline_input_validated, inline_multi_select,
    inline_select,
};
