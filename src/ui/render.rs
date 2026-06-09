//! Low-level rendering primitives.
//!
//! This module is the only place in the codebase that touches raw ANSI codes or
//! creates crossterm output.  All higher-level helpers in [`super::print`] and
//! [`super::widgets`] call through here so colour decisions always originate
//! from the active [`super::theme::Palette`].
//!
//! # What lives here
//!
//! - `ct_color` — converts `ratatui::style::Color` (used in the theme) to
//!   `crossterm::style::Color` (used for terminal output).
//! - `paint_*` — produce ANSI-encoded `String`s from a theme color + modifiers.
//! - [`Spinner`] / [`spinner`] — animated background-thread spinner driven by
//!   `ratatui_cheese::spinner::SpinnerState` for the frame sequence.
//! - [`ProgressBar`] / [`progress_bar`] — background-thread progress bar.
//! - `render_buffer_row` — converts a ratatui `Buffer` row to crossterm output;
//!   used by [`super::widgets`] to print ratatui-cheese widget output inline.

use std::io::{self, Write as IoWrite};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ─── No-interactive global flag ───────────────────────────────────────────────

/// When `true`, all interactive TUI prompts are suppressed and their default
/// values are returned immediately.  Set once at startup via
/// [`set_no_interactive`]; never mutated during a command run.
static NO_INTERACTIVE: AtomicBool = AtomicBool::new(false);

/// Activate no-interactive mode for the current process.
pub fn set_no_interactive() {
    NO_INTERACTIVE.store(true, Ordering::Relaxed);
}

/// Return `true` when `--no-interactive` was passed on the command line.
pub fn is_no_interactive() -> bool {
    NO_INTERACTIVE.load(Ordering::Relaxed)
}

/// When `true`, interactive prompts render inline (no alternate screen) instead
/// of switching to the full-screen ratatui TUI.
///
/// Set at startup from `[ui] prompt_mode = "inline"` in config, or at the
/// command level when `[ui] commit_mode = "inline"`.  Once set it is never
/// cleared within a single invocation.
static INLINE_PROMPTS: AtomicBool = AtomicBool::new(false);

/// Switch all interactive prompts to inline (non-fullscreen) mode.
///
/// Called at startup by `main::run` when `prompt_mode = "inline"` is
/// configured, or by individual commands that explicitly opt in to inline mode.
pub fn set_inline_prompts() {
    INLINE_PROMPTS.store(true, Ordering::Relaxed);
}

/// Return `true` when inline prompt mode is active.
pub fn is_inline_prompts() -> bool {
    INLINE_PROMPTS.load(Ordering::Relaxed)
}

use crossterm::style::{
    Attribute, Color as CtColor, ResetColor, SetAttribute, SetForegroundColor, Stylize,
};
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use ratatui_cheese::spinner::{Spinner as CheeseSpinner, SpinnerState, SpinnerType};

use super::theme;

// ─── Constants ────────────────────────────────────────────────────────────────

/// Standard left-margin applied to every output line.
///
/// Driven by the active theme's spacing (its `density`), so a theme can widen
/// or tighten the global indentation without any code changes.
#[inline]
pub fn indent() -> &'static str {
    theme::current().spacing.indent
}

// ─── Color conversion ─────────────────────────────────────────────────────────

/// Convert a `ratatui::style::Color` to the equivalent `crossterm::style::Color`.
///
/// The mapping matches the standard ANSI table, so `Color::Cyan` (ANSI 36)
/// produces the same terminal output as the old `colored` crate's `.cyan()`.
pub fn ct_color(c: Color) -> CtColor {
    match c {
        Color::Reset => CtColor::Reset,
        Color::Black => CtColor::Black,
        Color::Red => CtColor::DarkRed,
        Color::Green => CtColor::DarkGreen,
        Color::Yellow => CtColor::DarkYellow,
        Color::Blue => CtColor::DarkBlue,
        Color::Magenta => CtColor::DarkMagenta,
        Color::Cyan => CtColor::DarkCyan,
        Color::Gray => CtColor::Grey,
        Color::DarkGray => CtColor::DarkGrey,
        Color::LightRed => CtColor::Red,
        Color::LightGreen => CtColor::Green,
        Color::LightYellow => CtColor::Yellow,
        Color::LightBlue => CtColor::Blue,
        Color::LightMagenta => CtColor::Magenta,
        Color::LightCyan => CtColor::Cyan,
        Color::White => CtColor::White,
        Color::Rgb(r, g, b) => CtColor::Rgb { r, g, b },
        Color::Indexed(i) => CtColor::AnsiValue(i),
    }
}

// ─── Paint primitives ─────────────────────────────────────────────────────────

/// Apply a foreground color to `text`.
#[inline]
pub fn paint(text: &str, color: Color) -> String {
    text.with(ct_color(color)).to_string()
}

/// Apply foreground color + bold.
#[inline]
pub fn paint_bold(text: &str, color: Color) -> String {
    text.with(ct_color(color)).bold().to_string()
}

/// Apply foreground color + dim.
#[inline]
pub fn paint_dim(text: &str, color: Color) -> String {
    text.with(ct_color(color))
        .attribute(Attribute::Dim)
        .to_string()
}

/// Apply foreground color + underline.
#[inline]
pub fn paint_underline(text: &str, color: Color) -> String {
    text.with(ct_color(color)).underlined().to_string()
}

/// Apply foreground color + bold + underline.
#[inline]
pub fn paint_bold_underline(text: &str, color: Color) -> String {
    text.with(ct_color(color)).bold().underlined().to_string()
}

/// Render `text` using a reusable [`StyleSpec`] from the theme.
///
/// This is the building block for *component* rendering: a call site asks the
/// theme for a named style (`styles.section_title`) and paints with it, so the
/// color + modifiers travel together and can be re-skinned in one place.
#[inline]
pub fn paint_spec(text: &str, spec: theme::StyleSpec) -> String {
    let mut styled = text.with(ct_color(spec.color));
    if spec.bold {
        styled = styled.bold();
    }
    if spec.dim {
        styled = styled.attribute(Attribute::Dim);
    }
    if spec.underline {
        styled = styled.underlined();
    }
    styled.to_string()
}

// ─── Terminal geometry ────────────────────────────────────────────────────────

/// Return the terminal width in columns, falling back to 80.
pub fn terminal_width() -> usize {
    console::Term::stdout().size().1 as usize
}

// ─── Buffer → stdout ──────────────────────────────────────────────────────────

/// Print a single row of a ratatui [`Buffer`] to **stdout** using crossterm.
///
/// Used by [`super::widgets`] to render ratatui-cheese widget output inline
/// without creating a full `Terminal` instance.  Each cell's foreground color
/// and bold/dim modifiers are applied; everything resets after the row.
pub fn print_buffer_row(buffer: &Buffer, row_y: u16) {
    let mut out = io::stdout();
    for x in 0..buffer.area.width {
        let cell = match buffer.cell((x, row_y)) {
            Some(c) => c,
            None => continue,
        };
        let style = cell.style();

        if let Some(fg) = style.fg {
            if fg != Color::Reset {
                crossterm::execute!(out, SetForegroundColor(ct_color(fg))).ok();
            }
        }
        if style.add_modifier.contains(Modifier::BOLD) {
            crossterm::execute!(out, SetAttribute(Attribute::Bold)).ok();
        }
        if style.add_modifier.contains(Modifier::DIM) {
            crossterm::execute!(out, SetAttribute(Attribute::Dim)).ok();
        }
        crossterm::execute!(out, crossterm::style::Print(cell.symbol())).ok();
        crossterm::execute!(out, ResetColor, SetAttribute(Attribute::Reset)).ok();
    }
    crossterm::execute!(out, crossterm::style::Print("\n")).ok();
}

// ─── Spinner ──────────────────────────────────────────────────────────────────

/// An animated spinner that runs on a background thread.
///
/// Uses `ratatui_cheese::spinner::SpinnerState` for the frame sequence, then
/// renders each frame to a 1-cell ratatui [`Buffer`] and extracts the symbol
/// for crossterm output to stderr.  This keeps the spinner visually consistent
/// with any ratatui-cheese spinner used in full-screen TUI modes (Phase 3).
///
/// Create with [`spinner`].  Finish with [`Spinner::success`],
/// [`Spinner::error`], or [`Spinner::finish_and_clear`].  The [`Drop`]
/// implementation ensures cleanup even on early `?` propagation.
pub struct Spinner {
    done: Arc<AtomicBool>,
    message: Arc<Mutex<String>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Spinner {
    /// Update the spinner label while it is running.
    pub fn set_message(&self, msg: impl Into<String>) {
        if let Ok(mut m) = self.message.lock() {
            *m = msg.into();
        }
    }

    /// Stop the spinner and print a `✓` success line to stderr.
    pub fn success(mut self, msg: &str) {
        self.stop_thread();
        let t = theme::current();
        let icon = paint_bold(t.icons.success, t.palette.success);
        eprintln!("{} {} {}", indent(), icon, msg);
    }

    /// Stop the spinner and print a `✗` error line to stderr.
    pub fn error(mut self, msg: &str) {
        self.stop_thread();
        let t = theme::current();
        let icon = paint_bold(t.icons.error, t.palette.danger);
        eprintln!("{} {} {}", indent(), icon, msg);
    }

    /// Stop the spinner and clear its line silently.
    pub fn finish_and_clear(mut self) {
        self.stop_thread();
    }

    fn stop_thread(&mut self) {
        self.done.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            h.join().ok();
        }
        eprint!("\r\x1b[2K");
        io::stderr().flush().ok();
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        if self.handle.is_some() {
            self.stop_thread();
        }
    }
}

/// Start a spinner backed by `ratatui_cheese::spinner::SpinnerState`.
///
/// The spinner renders to stderr and runs until one of the finish methods is
/// called on the returned [`Spinner`] handle.
pub fn spinner(msg: &str) -> Spinner {
    let done = Arc::new(AtomicBool::new(false));
    let message = Arc::new(Mutex::new(msg.to_string()));

    let handle = {
        let done = done.clone();
        let message = message.clone();
        std::thread::spawn(move || {
            use ratatui::layout::Rect;
            use ratatui::widgets::StatefulWidget;

            let mut state = SpinnerState::new(SpinnerType::Dot);
            let start = Instant::now();
            let frame_area = Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            };
            let palette = &theme::current().palette;

            while !done.load(Ordering::Relaxed) {
                // Advance the ratatui-cheese spinner state by elapsed time.
                state.tick(start.elapsed());

                // Render the current frame into a 1-cell buffer.
                let mut buf = Buffer::empty(frame_area);
                CheeseSpinner::default()
                    // ratatui Style uses ratatui Color directly (not ct_color)
                    .style(ratatui::style::Style::default().fg(palette.primary))
                    .render(frame_area, &mut buf, &mut state);

                // Extract the rendered symbol (e.g. "⣾").
                let sym = buf
                    .cell((0, 0))
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_else(|| "⠋".to_string());

                let msg = message.lock().map(|m| m.clone()).unwrap_or_default();
                eprint!("\r{} {} {}", indent(), sym, msg);
                io::stderr().flush().ok();

                std::thread::sleep(Duration::from_millis(80));
            }
        })
    };

    Spinner {
        done,
        message,
        handle: Some(handle),
    }
}

/// Convenience wrapper: stop `s` and print a success message.
///
/// Kept for backward compatibility with call sites that use the free-function
/// form `ui::spinner_success(pb, msg)` rather than `pb.success(msg)`.
pub fn spinner_success(s: Spinner, msg: &str) {
    s.success(msg);
}

/// Convenience wrapper: stop `s` and print an error message.
pub fn spinner_error(s: Spinner, msg: &str) {
    s.error(msg);
}

// ─── ProgressBar ─────────────────────────────────────────────────────────────

/// A background-thread progress bar that renders to stderr.
///
/// Create with [`progress_bar`].  Increment with [`ProgressBar::inc`],
/// update the label with [`ProgressBar::set_message`], and finish with
/// [`ProgressBar::finish_and_clear`].
pub struct ProgressBar {
    done: Arc<AtomicBool>,
    current: Arc<AtomicU64>,
    #[allow(dead_code)]
    total: u64,
    message: Arc<Mutex<String>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl ProgressBar {
    /// Increment the step counter by `n`.
    pub fn inc(&self, n: u64) {
        self.current.fetch_add(n, Ordering::Relaxed);
    }

    /// Update the label shown beside the bar.
    pub fn set_message(&self, msg: impl Into<String>) {
        if let Ok(mut m) = self.message.lock() {
            *m = msg.into();
        }
    }

    /// Stop the bar and erase its line.
    pub fn finish_and_clear(mut self) {
        self.stop_thread();
    }

    fn stop_thread(&mut self) {
        self.done.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            h.join().ok();
        }
        eprint!("\r\x1b[2K");
        io::stderr().flush().ok();
    }
}

impl Drop for ProgressBar {
    fn drop(&mut self) {
        if self.handle.is_some() {
            self.stop_thread();
        }
    }
}

/// Create a progress bar for `total` steps with an initial label.
pub fn progress_bar(total: u64, msg: &str) -> ProgressBar {
    let done = Arc::new(AtomicBool::new(false));
    let current = Arc::new(AtomicU64::new(0));
    let message = Arc::new(Mutex::new(msg.to_string()));

    let handle = {
        let done = done.clone();
        let current = current.clone();
        let message = message.clone();
        std::thread::spawn(move || {
            use ratatui::layout::Rect;
            use ratatui::widgets::StatefulWidget;

            const BAR_WIDTH: usize = 30;
            let palette = &theme::current().palette;
            let mut state = SpinnerState::new(SpinnerType::Dot);
            let start = Instant::now();
            let frame_area = Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            };

            while !done.load(Ordering::Relaxed) {
                state.tick(start.elapsed());
                let mut buf = Buffer::empty(frame_area);
                CheeseSpinner::default()
                    .style(ratatui::style::Style::default().fg(palette.primary))
                    .render(frame_area, &mut buf, &mut state);
                let sym = buf
                    .cell((0, 0))
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_else(|| "⠋".to_string());

                let cur = current.load(Ordering::Relaxed);
                let msg = message.lock().map(|m| m.clone()).unwrap_or_default();
                let pct = if total > 0 {
                    ((cur * BAR_WIDTH as u64) / total) as usize
                } else {
                    0
                }
                .min(BAR_WIDTH);

                let bar = format!(
                    "{}{}",
                    paint(&"█".repeat(pct), palette.primary),
                    paint_dim(&"░".repeat(BAR_WIDTH - pct), palette.muted)
                );
                eprint!(
                    "\r{} {} [{}] {}/{}  {}",
                    indent(),
                    sym,
                    bar,
                    cur,
                    total,
                    paint_dim(&msg, palette.muted)
                );
                io::stderr().flush().ok();
                std::thread::sleep(Duration::from_millis(100));
            }
        })
    };

    ProgressBar {
        done,
        current,
        total,
        message,
        handle: Some(handle),
    }
}
