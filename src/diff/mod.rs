//! Builtin diff engine — parsing, syntax highlighting, renderers, and review
//! comments.
//!
//! In-process pipeline that produces a typed [`parse::FileDiff`] tree,
//! highlights each line with [`syntect`], and renders it through one of three
//! layouts inside the same ratatui TUI (stack / split / side-by-side) or as
//! inline ANSI for non-interactive / non-TTY use.  [`reviews`] provides the
//! two-bucket review-comment coordination (public → GitHub, private → SQLite).
//!
//! # Folder layout
//!
//! ```text
//! diff/
//!   mod.rs            ← this file: module root
//!   parse.rs          ← unified-diff parser (pure)
//!   highlight.rs      ← syntect engine + per-line span builder
//!   theme.rs          ← UI theme → syntect::Theme bridge
//!   render_inline.rs  ← ANSI renderer (non-TTY / --raw)
//!   render_tui.rs     ← ratatui app: layouts, sidebar, key loop
//!   reviews.rs        ← review-comment coordination (storage + GitHub)
//! ```
//!
//! Phase 1 scope: stack / split / side-by-side + inline fallback.
//! Phase 2: review comments — public PR + private local notes.

pub mod highlight;
pub mod parse;
pub mod render_inline;
pub mod render_tui;
pub mod reviews;
pub mod theme;

// Callers reach parse types directly as `diff::parse::FileDiff`, etc.  No
// blanket re-exports here — that would leave unused-symbol warnings behind.