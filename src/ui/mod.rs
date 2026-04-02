//! Terminal UI helpers — colors, tables, spinners, and git-specific formatting.
//!
//! ## Tutorial overview
//!
//! This module centralises all presentation logic so every command has a
//! consistent look and feel.  It provides:
//!
//! - High-level "message" helpers (`print_info`, `print_success`, …).
//! - Layout helpers (`print_blank`, `print_rule`, `print_step`, `print_header`).
//! - A flexible [`Table`] renderer that accounts for ANSI color codes when
//!   calculating column widths.
//! - A [`CommitEntry`] value type for rendering individual git log entries.
//! - Spinners and progress bars via the `indicatif` crate, including
//!   `spinner_success` / `spinner_error` for clean finish-in-place transitions.
//! - Colour helpers for git-specific data: hashes, branch names, commit
//!   subjects following the Conventional Commits convention, etc.
//!
//! ## Rust concepts used here
//!
//! - Traits like `std::fmt::Write` for efficient string building without heap
//!   allocation on every `format!`.
//! - `match` expressions for mapping status codes and commit types to icons.
//! - `indicatif` for progress bars and spinners.
//! - `console` for measuring *visible* text width (ignoring ANSI escape codes).
//! - `struct` with `impl` blocks for stateful UI components like [`Table`].

use colored::Colorize;
use std::fmt::Write as FmtWrite;

// ─── Layout constants ─────────────────────────────────────────────────────────

/// Standard left-margin indent applied to all output lines.
pub const INDENT: &str = "  ";

// ─── Terminal geometry ────────────────────────────────────────────────────────

/// Return the current terminal width in columns, falling back to 80 if the
/// width cannot be determined (e.g. when stdout is not a TTY).
#[allow(dead_code)]
pub fn terminal_width() -> usize {
    console::Term::stdout().size().1 as usize
}

// ─── Status-message helpers ──────────────────────────────────────────────────

/// Print an info message with a cyan bullet to stdout.
pub fn print_info(msg: &str) {
    println!("{} {} {}", INDENT, "ℹ".cyan(), msg);
}

/// Print a success message with a bold green checkmark to stdout.
pub fn print_success(msg: &str) {
    println!("{} {} {}", INDENT, "✓".green().bold(), msg);
}

/// Print a warning message with a bold yellow warning sign to stderr.
pub fn print_warning(msg: &str) {
    eprintln!("{} {} {}", INDENT, "⚠".yellow().bold(), msg);
}

/// Print an error message with a bold red × to stderr.
pub fn print_error(msg: &str) {
    eprintln!("{} {} {}", INDENT, "✗".red().bold(), msg);
}

/// Print a dim `tip:` hint line to stdout.
///
/// This is a companion to [`print_info`] for short, actionable hints shown
/// after a command completes.  Centralising it here ensures the `"tip:"`
/// prefix is always styled the same way across all commands.
///
/// # Example
///
/// ```text
/// print_tip("g commit  — commit staged changes");
/// // →   tip: ▶ g commit  — commit staged changes
/// ```
pub fn print_tip(msg: &str) {
    println!(
        "{} {}  {}",
        INDENT,
        "tip:".bright_black().bold(),
        msg.bright_black()
    );
}

/// Print a blank line to stdout.
#[allow(dead_code)]
pub fn print_blank() {
    println!();
}

/// Print a full-width horizontal rule scaled to the terminal width.
///
/// Falls back to 60 characters when the terminal width cannot be detected.
#[allow(dead_code)]
pub fn print_rule() {
    let width = terminal_width().saturating_sub(INDENT.len()).max(10);
    println!("{}{}", INDENT, "─".repeat(width).bright_black());
}

/// Print a numbered step indicator for multi-step operations.
///
/// # Example
///
/// ```text
/// print_step(1, 3, "Fetching remote…");
/// // →   [1/3] Fetching remote…
/// ```
#[allow(dead_code)]
pub fn print_step(step: usize, total: usize, msg: &str) {
    let counter = format!("[{}/{}]", step, total)
        .bright_black()
        .bold()
        .to_string();
    println!("{}{} {}", INDENT, counter, msg);
}

/// Print a set of key-value pairs with the value column auto-aligned.
///
/// Keys are rendered in dim grey; values are rendered as-is (ANSI codes in the
/// value strings are respected).  All keys are measured with plain `len()` since
/// they are expected to be plain ASCII without escape codes.
///
/// # Example
///
/// ```text
/// print_key_value_pairs(&[
///     ("Config file", "/path/to/config.toml".cyan().underline().to_string()),
///     ("Default branch", "main".green().to_string()),
/// ]);
/// ```
pub fn print_key_value_pairs(pairs: &[(&str, String)]) {
    let max_key = pairs.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (key, val) in pairs {
        let padding = " ".repeat(max_key - key.len());
        println!(
            "{}{}{} {}  {}",
            INDENT,
            key.bright_black(),
            padding,
            " ".bright_black(),
            val
        );
    }
}

// ─── Branch / stack markers ───────────────────────────────────────────────────

/// Return the Unicode marker (`◉` / `◯`) for a branch row, coloured by state.
///
/// `◉` (filled) in bold green marks the currently checked-out branch;
/// `◯` (empty) in dim grey marks any other branch.
///
/// Keeping this in one place means every tree view (stack list, workspace list,
/// branch list) uses the same symbols consistently.
pub fn branch_marker(is_current: bool) -> String {
    if is_current {
        "◉".green().bold().to_string()
    } else {
        "◯".bright_black().to_string()
    }
}

/// Return `name` coloured for its role in a branch-tree row.
///
/// The current branch is bold green; all others are plain white.
pub fn branch_name_colored(name: &str, is_current: bool) -> String {
    if is_current {
        name.green().bold().to_string()
    } else {
        name.white().to_string()
    }
}

/// Print the standard "verb stack: <name>" banner used by `push`, `sync`, and `pr`.
///
/// All three stack operations printed the same three-line block inline.  This
/// helper gives it a single source of truth.
pub fn print_stack_banner(verb: &str, stack_name: &str) {
    println!();
    println!(
        "{}  {} {}",
        INDENT,
        verb.bold().white(),
        stack_name.cyan().bold()
    );
    println!();
}

/// Print a section header inside a Unicode box to stdout.
///
/// The box width adjusts to the title length.  Left-margin indent is applied
/// so the box aligns with all other output.
#[allow(dead_code)]
pub fn print_header(title: &str) {
    let title_width = console::measure_text_width(title);
    let inner = title_width + 2; // one space padding on each side
    let line = "─".repeat(inner);
    println!("{}{}", INDENT, format!("╭{}╮", line).bright_black());
    println!(
        "{}{} {} {}",
        INDENT,
        "│".bright_black(),
        title.bold().white(),
        "│".bright_black()
    );
    println!("{}{}", INDENT, format!("╰{}╯", line).bright_black());
}

/// Print a section title with an optional item count in parentheses.
///
/// A blank line is emitted before the title so sections are visually separated.
///
/// # Examples
///
/// ```text
/// print_section("Staged Changes", Some(3));
/// // →
/// // →   Staged Changes (3)
/// ```
pub fn print_section(title: &str, count: Option<usize>) {
    if let Some(n) = count {
        println!(
            "\n{} {} {}",
            INDENT,
            title.bold().white(),
            format!("({})", n).bright_black()
        );
    } else {
        println!("\n{} {}", INDENT, title.bold().white());
    }
}

/// Print a horizontal divider line in dim colour.
///
/// Delegates to [`print_rule`] so the width tracks the terminal.
#[allow(dead_code)]
pub fn print_divider() {
    print_rule();
}

// ─── Git colour helpers ───────────────────────────────────────────────────────

/// Colour a git commit hash (short or long) with yellow+dimmed styling.
pub fn color_hash(hash: &str) -> String {
    hash.yellow().dimmed().to_string()
}

/// Colour a branch name based on its type.
///
/// - Remote branches (`origin/…`, `upstream/…`) → bold red.
/// - `HEAD` → bold cyan.
/// - Local branches → bold green.
pub fn color_branch(name: &str) -> String {
    if name.starts_with("origin/") || name.starts_with("upstream/") {
        name.red().bold().to_string()
    } else if name == "HEAD" {
        name.cyan().bold().to_string()
    } else {
        name.green().bold().to_string()
    }
}

/// Colour a ref decoration string (tags, remotes, HEAD pointers, etc.).
pub fn color_ref(r: &str) -> String {
    if r.contains("HEAD") {
        r.cyan().bold().to_string()
    } else if r.starts_with("tag:") {
        r.yellow().bold().to_string()
    } else if r.contains('/') {
        r.red().to_string()
    } else {
        r.green().bold().to_string()
    }
}

/// Colour an author name in cyan.
pub fn color_author(name: &str) -> String {
    name.cyan().to_string()
}

/// Colour a date string in dim grey.
pub fn color_date(date: &str) -> String {
    date.bright_black().to_string()
}

/// Colour a commit subject, highlighting Conventional Commit prefixes.
///
/// If the subject contains a `:`, the part before the colon is treated as the
/// commit type and coloured according to the table below:
///
/// | Type prefix | Colour |
/// |-------------|--------|
/// | `feat`      | bold green |
/// | `fix`       | bold red |
/// | `docs`      | bold blue |
/// | `refactor`  | bold magenta |
/// | `perf`      | bold yellow |
/// | `test`      | bold cyan |
/// | `chore` / `build` / `ci` | dim grey |
/// | `revert`    | dim red |
/// | *other*     | bold white |
pub fn color_subject(subject: &str) -> String {
    if let Some(idx) = subject.find(':') {
        let prefix = &subject[..idx];
        let rest = &subject[idx..];
        let colored_prefix = if prefix.starts_with("feat") {
            prefix.green().bold().to_string()
        } else if prefix.starts_with("fix") {
            prefix.red().bold().to_string()
        } else if prefix.starts_with("docs") {
            prefix.blue().bold().to_string()
        } else if prefix.starts_with("refactor") {
            prefix.magenta().bold().to_string()
        } else if prefix.starts_with("perf") {
            prefix.yellow().bold().to_string()
        } else if prefix.starts_with("test") {
            prefix.cyan().bold().to_string()
        } else if prefix.starts_with("chore")
            || prefix.starts_with("build")
            || prefix.starts_with("ci")
        {
            prefix.bright_black().bold().to_string()
        } else if prefix.starts_with("revert") {
            prefix.red().dimmed().to_string()
        } else {
            prefix.white().bold().to_string()
        };
        format!("{}{}", colored_prefix, rest.white())
    } else {
        subject.white().to_string()
    }
}

/// Render a green `+N` added-lines count.
pub fn color_added(n: i64) -> String {
    format!("+{}", n).green().to_string()
}

/// Render a red `-N` deleted-lines count.
pub fn color_deleted(n: i64) -> String {
    format!("-{}", n).red().to_string()
}

// ─── Status icons ─────────────────────────────────────────────────────────────

/// Convert a git porcelain status code into an `(icon, coloured_code)` pair.
///
/// The icon is a static `&str` (single Unicode character); the coloured code
/// is an owned `String` already formatted with ANSI codes.
pub fn status_icon(code: &str) -> (&'static str, String) {
    match code {
        "A" | "AA" => ("✚", "A".green().bold().to_string()),
        "M" | "MM" => ("✎", "M".yellow().bold().to_string()),
        "D" | "DD" => ("✖", "D".red().bold().to_string()),
        "R" | "RR" => ("➜", "R".cyan().bold().to_string()),
        "C" | "CC" => ("⊕", "C".cyan().to_string()),
        "U" | "UU" => ("⚡", "U".red().bold().to_string()),
        "?" => ("?", "?".bright_black().to_string()),
        "!" => ("!", "!".bright_black().to_string()),
        _ => ("·", code.bright_black().to_string()),
    }
}

// ─── Progress / spinner ───────────────────────────────────────────────────────

/// Braille tick characters used by all spinners for a consistent animation.
const SPINNER_TICKS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Build the shared spinner style so all spinners look identical.
fn spinner_style() -> indicatif::ProgressStyle {
    indicatif::ProgressStyle::with_template(&format!("{}{{spinner:.cyan}} {{msg}}", INDENT))
        .expect("spinner template is valid")
        .tick_strings(SPINNER_TICKS)
}

/// Build the minimal "finish" style used by [`spinner_success`] and
/// [`spinner_error`] to replace the animated line with a static result.
fn spinner_finish_style() -> indicatif::ProgressStyle {
    indicatif::ProgressStyle::with_template(&format!("{} {{msg}}", INDENT))
        .expect("spinner finish template is valid")
}

/// Create and start a braille-spinner progress bar with `msg` as its label.
///
/// The spinner ticks automatically every 80 ms.  Finish with one of:
/// - [`spinner_success`] — replaces spinner with a `✓` success line.
/// - [`spinner_error`]   — replaces spinner with a `✗` error line.
/// - `.finish_and_clear()` — removes the spinner silently.
pub fn spinner(msg: &str) -> indicatif::ProgressBar {
    let pb = indicatif::ProgressBar::new_spinner();
    pb.set_style(spinner_style());
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

/// Finish a spinner in-place with a bold green `✓` success message.
///
/// The animated spinner line is replaced with a static success line so there
/// is no flicker or blank-line gap between the spinner and the result.
///
/// # Example
///
/// ```rust,ignore
/// let pb = ui::spinner("Pushing…");
/// do_work();
/// ui::spinner_success(pb, "Pushed to origin");
/// ```
pub fn spinner_success(pb: indicatif::ProgressBar, msg: &str) {
    pb.set_style(spinner_finish_style());
    pb.finish_with_message(format!("{} {}", "✓".green().bold(), msg));
}

/// Finish a spinner in-place with a bold red `✗` error message.
///
/// Use this when the operation the spinner was tracking has failed, so the
/// terminal shows an error icon without requiring a separate `print_error` call.
///
/// # Example
///
/// ```rust,ignore
/// let pb = ui::spinner("Pushing…");
/// if let Err(e) = do_work() {
///     ui::spinner_error(pb, &format!("Push failed: {}", e));
/// }
/// ```
pub fn spinner_error(pb: indicatif::ProgressBar, msg: &str) {
    pb.set_style(spinner_finish_style());
    pb.finish_with_message(format!("{} {}", "✗".red().bold(), msg));
}

/// Create a [`indicatif::MultiProgress`] for displaying several concurrent
/// spinners or bars without interleaving their output.
///
/// Add individual spinners with [`multi_spinner`].
#[allow(dead_code)]
pub fn multi_progress() -> indicatif::MultiProgress {
    indicatif::MultiProgress::new()
}

/// Add a new spinner to an existing [`indicatif::MultiProgress`] group.
///
/// The spinner starts ticking immediately.  Finish it with [`spinner_success`]
/// or [`spinner_error`] as you would a standalone spinner.
#[allow(dead_code)]
pub fn multi_spinner(mp: &indicatif::MultiProgress, msg: &str) -> indicatif::ProgressBar {
    let pb = mp.add(indicatif::ProgressBar::new_spinner());
    pb.set_style(spinner_style());
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

/// Create a fixed-length progress bar with `len` steps and `msg` as its label.
///
/// The bar shows the current position, total, estimated time remaining, and
/// a smooth Unicode block-fill animation.
pub fn progress_bar(len: u64, msg: &str) -> indicatif::ProgressBar {
    let pb = indicatif::ProgressBar::new(len);
    pb.set_style(
        indicatif::ProgressStyle::with_template(&format!(
            "{}{{spinner:.cyan}} [{{bar:40.cyan/blue}}] {{pos}}/{{len}}  {{eta_precise}}  {{msg}}",
            INDENT
        ))
        .expect("progress bar template is valid")
        .progress_chars("█▉▊▋▌▍▎▏  "),
    );
    pb.set_message(msg.to_string());
    pb
}

// ─── Diff stat bar ────────────────────────────────────────────────────────────

/// Render a fixed-`width` colour bar showing added-vs-deleted proportions.
///
/// Green blocks represent additions; red blocks represent deletions.
/// Returns an empty string when both counts are zero.
pub fn render_stat_bar(added: usize, deleted: usize, width: usize) -> String {
    let total = added + deleted;
    if total == 0 {
        return String::new();
    }
    let add_blocks = (added * width / total).max(if added > 0 { 1 } else { 0 });
    let del_blocks = (width - add_blocks).min(deleted.min(width));

    format!(
        "{}{}",
        "█".repeat(add_blocks).green(),
        "█".repeat(del_blocks).red(),
    )
}

// ─── Ref decoration formatter ─────────────────────────────────────────────────

/// Format git ref decorations (`HEAD -> main, origin/main`) into coloured badges.
///
/// Returns an empty string when `refs_str` is blank.
pub fn format_refs(refs_str: &str) -> String {
    if refs_str.trim().is_empty() {
        return String::new();
    }
    let parts: Vec<&str> = refs_str.split(',').map(str::trim).collect();
    let formatted: Vec<String> = parts
        .iter()
        .filter(|r| !r.is_empty())
        .map(|r| {
            if r.starts_with("HEAD ->") {
                let branch = r.trim_start_matches("HEAD ->").trim();
                format!(
                    "{} {} {}",
                    "HEAD →".cyan().bold(),
                    "".bright_black(),
                    color_branch(branch)
                )
            } else {
                color_ref(r)
            }
        })
        .collect();
    if formatted.is_empty() {
        String::new()
    } else {
        format!(
            " {} {} {}",
            "(".bright_black(),
            formatted.join(&" · ".bright_black().to_string()),
            ")".bright_black()
        )
    }
}

// ─── Table formatter ──────────────────────────────────────────────────────────

/// A simple columnar table renderer that accounts for ANSI escape codes when
/// measuring cell widths.
///
/// ## Example
///
/// ```text
/// let mut t = Table::new(vec!["Name", "Branch"]);
/// t.add_row(vec!["feature-x".to_string(), "green-branch".to_string()]);
/// t.print();
/// ```
pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    col_widths: Vec<usize>,
}

impl Table {
    /// Create a new table with the provided header labels.
    ///
    /// Column widths are initialised to the visible width of each header.
    pub fn new(headers: Vec<&str>) -> Self {
        let col_widths = headers
            .iter()
            .map(|h| console::measure_text_width(h))
            .collect();
        Self {
            headers: headers.into_iter().map(String::from).collect(),
            rows: vec![],
            col_widths,
        }
    }

    /// Append a row, automatically expanding column widths as needed.
    pub fn add_row(&mut self, row: Vec<String>) {
        for (i, cell) in row.iter().enumerate() {
            let visible_width = console::measure_text_width(cell);
            if i < self.col_widths.len() {
                self.col_widths[i] = self.col_widths[i].max(visible_width);
            }
        }
        self.rows.push(row);
    }

    /// Print the table — headers, a divider, then each row — to stdout.
    pub fn print(&self) {
        // Pad a cell to the target column width, accounting for invisible ANSI codes.
        let pad_cell = |cell: &str, col: usize| -> String {
            let visible_width = console::measure_text_width(cell);
            let target = self.col_widths.get(col).copied().unwrap_or(0);
            let padding = target.saturating_sub(visible_width);
            format!("{}{}", cell, " ".repeat(padding))
        };

        // Header row.
        let header_cells: Vec<String> = self
            .headers
            .iter()
            .enumerate()
            .map(|(i, h)| pad_cell(&h.bold().bright_white().to_string(), i))
            .collect();
        println!("{}{}", INDENT, header_cells.join("  "));

        // Divider.
        let divider: Vec<String> = self
            .col_widths
            .iter()
            .map(|w| "─".repeat(*w).bright_black().to_string())
            .collect();
        println!("{}{}", INDENT, divider.join("  "));

        // Data rows.
        for row in &self.rows {
            let cells: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(i, cell)| pad_cell(cell, i))
                .collect();
            println!("{}{}", INDENT, cells.join("  "));
        }
    }
}

// ─── Branch ahead/behind ─────────────────────────────────────────────────────

/// Format ahead/behind commit counts into a compact, coloured string.
///
/// | State          | Output example          |
/// |----------------|-------------------------|
/// | Both zero      | `up to date` (dim)      |
/// | Ahead only     | `↑ 3 ahead` (green)     |
/// | Behind only    | `↓ 2 behind` (red)      |
/// | Both non-zero  | `↑ 3 ahead  ↓ 2 behind` |
pub fn format_ahead_behind(ahead: usize, behind: usize) -> String {
    match (ahead, behind) {
        (0, 0) => "up to date".bright_black().to_string(),
        (a, 0) => format!("{} {}", "↑".green(), format!("{} ahead", a).green()),
        (0, b) => format!("{} {}", "↓".red(), format!("{} behind", b).red()),
        (a, b) => format!(
            "{} {}  {} {}",
            "↑".green(),
            format!("{} ahead", a).green(),
            "↓".red(),
            format!("{} behind", b).red()
        ),
    }
}

// ─── Stack tree ───────────────────────────────────────────────────────────────

/// Print a stack tree to stdout.
///
/// `branches` is a slice of `(name, is_current, pr_url)` tuples.
#[allow(dead_code)]
pub fn print_stack_tree(stack_name: &str, branches: &[(String, bool, Option<String>)]) {
    println!(
        "\n{}  {} {}",
        INDENT,
        "Stack:".bold().bright_white(),
        stack_name.cyan().bold()
    );
    println!();
    let last = branches.len().saturating_sub(1);
    for (i, (branch, is_current, pr_url)) in branches.iter().enumerate() {
        let connector = if i == last { "└" } else { "├" };
        let pipe = if i == last { " " } else { "│" };
        let marker = if *is_current {
            "◉".green().bold().to_string()
        } else {
            "◯".bright_black().to_string()
        };

        print!(
            "{}{}── {} {}",
            INDENT,
            connector.bright_black(),
            marker,
            if *is_current {
                branch.green().bold().to_string()
            } else {
                branch.white().to_string()
            }
        );

        if let Some(url) = pr_url {
            print!("  {}", url.bright_black().underline());
        }
        println!();

        if i < last {
            println!("{}{}   {}", INDENT, pipe.bright_black(), "│".bright_black());
        }
    }
    println!();
}

// ─── Commit entry ─────────────────────────────────────────────────────────────

/// A single git log entry ready to be rendered to a terminal line.
///
/// Construct this struct from the fields parsed out of `git log --format=…`,
/// then call [`CommitEntry::render`] to obtain the final coloured string.
pub struct CommitEntry {
    /// Short (7-char) commit hash.
    pub hash: String,
    /// Commit subject (first line of the commit message).
    pub subject: String,
    /// Author name.
    pub author: String,
    /// Relative date string (e.g. "3 days ago").
    pub date: String,
    /// Raw ref decorations string from `%D` (e.g. `HEAD -> main, origin/main`).
    pub refs: String,
    /// ASCII graph prefix from `git log --graph` (may be empty).
    pub graph_prefix: String,
}

impl CommitEntry {
    /// Render a single log line with padding and coloured fields.
    ///
    /// `max_subject` is the maximum *visible* character width reserved for the
    /// subject column; shorter subjects are padded with spaces so columns align.
    pub fn render(&self, max_subject: usize) -> String {
        let mut out = String::new();

        // Graph prefix (git's graph art, colorised).
        if !self.graph_prefix.is_empty() {
            let colored_graph = colorize_graph(&self.graph_prefix);
            write!(out, "{}", colored_graph).ok();
        }

        // Hash.
        write!(out, " {} ", color_hash(&self.hash)).ok();

        // Subject — truncated and padded to a fixed display width.
        let subject = truncate(&self.subject, max_subject);
        let colored_subject = color_subject(&subject);
        let subject_width = console::measure_text_width(&colored_subject);
        let subject_pad = max_subject.saturating_sub(subject_width);
        write!(out, "{}{}", colored_subject, " ".repeat(subject_pad)).ok();

        // Author — truncated and padded to a fixed display width.
        let author_max = 20;
        let author = truncate(&self.author, author_max);
        let colored_author = color_author(&author);
        let author_width = console::measure_text_width(&colored_author);
        let author_pad = author_max.saturating_sub(author_width);
        write!(out, "  {}{}", colored_author, " ".repeat(author_pad)).ok();

        // Date.
        write!(out, "  {}", color_date(&self.date)).ok();

        // Ref decorations.
        if !self.refs.trim().is_empty() {
            write!(out, " {}", format_refs(&self.refs)).ok();
        }

        out
    }
}

/// Truncate a string to a maximum *visible* display width, appending `…` if needed.
fn truncate(s: &str, max: usize) -> String {
    if console::measure_text_width(s) > max {
        let truncated = console::truncate_str(s, max.saturating_sub(1), "");
        format!("{}…", truncated)
    } else {
        s.to_string()
    }
}

/// Colorise the ASCII graph output produced by `git log --graph`.
///
/// Each graph character (`*`, `|`, `/`, `\`, `-`) is wrapped in an ANSI colour
/// escape sequence, cycling through a small palette to distinguish parallel
/// branch lines.
pub fn colorize_graph(graph: &str) -> String {
    // A small palette of ANSI colour codes.  We cycle through them based on the
    // column position so that adjacent parallel lines get different colours.
    let colors = [
        "\x1b[33m", // yellow
        "\x1b[32m", // green
        "\x1b[34m", // blue
        "\x1b[35m", // magenta
        "\x1b[36m", // cyan
    ];
    let reset = "\x1b[0m";
    let mut result = String::new();
    for (i, ch) in graph.chars().enumerate() {
        match ch {
            '*' => {
                result.push_str(&format!("{}{}{}", colors[0], ch, reset));
            }
            '|' => {
                let col_idx = (i / 2) % colors.len();
                result.push_str(&format!("{}{}{}", colors[col_idx], ch, reset));
            }
            '/' | '\\' => {
                let col_idx = (i / 2) % colors.len();
                result.push_str(&format!("{}{}{}", colors[col_idx], ch, reset));
            }
            '-' => result.push_str(&format!("{}{}{}", "\x1b[90m", ch, reset)),
            _ => result.push(ch),
        }
    }
    result
}
