//! Terminal UI helpers (colors, tables, formatting).
//!
//! These functions keep presentation logic in one place so the command
//! modules can focus on business logic.

use colored::Colorize;
use std::fmt::Write as FmtWrite;

// ─── Theme Colors ─────────────────────────────────────────────────────────────

/// Print an info message with a cyan bullet
/// Print an info message with a cyan bullet.
pub fn print_info(msg: &str) {
    println!("  {} {}", "ℹ".cyan(), msg);
}

/// Print a success message with a green checkmark
/// Print a success message with a green checkmark.
pub fn print_success(msg: &str) {
    println!("  {} {}", "✓".green().bold(), msg);
}

/// Print a warning message with a yellow warning sign
/// Print a warning message with a yellow warning sign.
pub fn print_warning(msg: &str) {
    eprintln!("  {} {}", "⚠".yellow().bold(), msg);
}

/// Print an error message with a red X
/// Print an error message with a red X.
pub fn print_error(msg: &str) {
    eprintln!("  {} {}", "✗".red().bold(), msg);
}

/// Print a section header in a box
/// Print a section header in a box.
#[allow(dead_code)]
pub fn print_header(title: &str) {
    let width = title.len() + 4;
    let line = "─".repeat(width);
    println!("{}", format!("╭{}╮", line).bright_black());
    println!(
        "{} {} {}",
        "│".bright_black(),
        title.bold().white(),
        "│".bright_black()
    );
    println!("{}", format!("╰{}╯", line).bright_black());
}

/// Print a section title (lighter than header)
/// Print a section title (lighter than header).
pub fn print_section(title: &str, count: Option<usize>) {
    if let Some(n) = count {
        println!(
            "\n  {} {}",
            title.bold().white(),
            format!("({})", n).bright_black()
        );
    } else {
        println!("\n  {}", title.bold().white());
    }
}

/// Divider line
/// Divider line.
#[allow(dead_code)]
pub fn print_divider() {
    println!("  {}", "─".repeat(60).bright_black());
}

// ─── Git Color Helpers ────────────────────────────────────────────────────────

/// Color a git hash.
pub fn color_hash(hash: &str) -> String {
    hash.yellow().dimmed().to_string()
}

/// Color a branch name based on its type (local/remote/HEAD).
pub fn color_branch(name: &str) -> String {
    if name.starts_with("origin/") || name.starts_with("upstream/") {
        name.red().bold().to_string()
    } else if name == "HEAD" {
        name.cyan().bold().to_string()
    } else {
        name.green().bold().to_string()
    }
}

/// Color a ref decoration (tags, remotes, HEAD, etc.).
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

/// Color an author name.
pub fn color_author(name: &str) -> String {
    name.cyan().to_string()
}

/// Color a date string.
pub fn color_date(date: &str) -> String {
    date.bright_black().to_string()
}

/// Color a commit subject, highlighting Conventional Commit prefixes.
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

/// Render a green "+N" added count.
pub fn color_added(n: i64) -> String {
    format!("+{}", n).green().to_string()
}

/// Render a red "-N" deleted count.
pub fn color_deleted(n: i64) -> String {
    format!("-{}", n).red().to_string()
}

// ─── Status Icons ─────────────────────────────────────────────────────────────

/// Convert porcelain status code to an icon + colored code.
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

// ─── Progress / Spinner ───────────────────────────────────────────────────────

/// Create a spinner progress bar with a custom message.
pub fn spinner(msg: &str) -> indicatif::ProgressBar {
    let pb = indicatif::ProgressBar::new_spinner();
    pb.set_style(
        indicatif::ProgressStyle::with_template("  {spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

/// Create a progress bar with a fixed length and message.
#[allow(dead_code)]
pub fn progress_bar(len: u64, msg: &str) -> indicatif::ProgressBar {
    let pb = indicatif::ProgressBar::new(len);
    pb.set_style(
        indicatif::ProgressStyle::with_template(
            "  {spinner:.cyan} [{bar:40.cyan/blue}] {pos}/{len} {msg}",
        )
        .unwrap()
        .progress_chars("█▉▊▋▌▍▎▏  "),
    );
    pb.set_message(msg.to_string());
    pb
}

// ─── Diff Stat Bar ────────────────────────────────────────────────────────────

/// Render a fixed-width bar showing added vs deleted proportions.
pub fn render_stat_bar(added: usize, deleted: usize, width: usize) -> String {
    let total = added + deleted;
    if total == 0 {
        return String::new();
    }
    let add_blocks = if total > 0 {
        (added * width / total).max(if added > 0 { 1 } else { 0 })
    } else {
        0
    };
    let del_blocks = (width - add_blocks).min(deleted.min(width));

    format!(
        "{}{}",
        "█".repeat(add_blocks).green(),
        "█".repeat(del_blocks).red(),
    )
}

// ─── Ref Decoration Formatter ─────────────────────────────────────────────────

/// Format git ref decorations (HEAD -> main, origin/main) into colored badges.
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

// ─── Table Formatter ─────────────────────────────────────────────────────────

/// Simple table renderer that accounts for ANSI color codes.
pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    col_widths: Vec<usize>,
}

impl Table {
    /// Create a new table with the provided header labels.
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

    /// Add a row to the table (updates column widths).
    pub fn add_row(&mut self, row: Vec<String>) {
        for (i, cell) in row.iter().enumerate() {
            let visible_width = console::measure_text_width(cell);
            if i < self.col_widths.len() {
                self.col_widths[i] = self.col_widths[i].max(visible_width);
            }
        }
        self.rows.push(row);
    }

    /// Print the table to stdout.
    pub fn print(&self) {
        let pad_cell = |cell: &str, col: usize| -> String {
            let visible_width = console::measure_text_width(cell);
            let target = self.col_widths.get(col).copied().unwrap_or(0);
            let padding = target.saturating_sub(visible_width);
            format!("{}{}", cell, " ".repeat(padding))
        };

        // Header
        let header_cells: Vec<String> = self
            .headers
            .iter()
            .enumerate()
            .map(|(i, h)| pad_cell(&h.bold().bright_white().to_string(), i))
            .collect();
        println!("  {}", header_cells.join("  "));

        // Divider
        let divider: Vec<String> = self
            .col_widths
            .iter()
            .map(|w| "─".repeat(*w).bright_black().to_string())
            .collect();
        println!("  {}", divider.join("  "));

        // Rows
        for row in &self.rows {
            let cells: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(i, cell)| pad_cell(cell, i))
                .collect();
            println!("  {}", cells.join("  "));
        }
    }
}

// ─── Branch Ahead/Behind ─────────────────────────────────────────────────────

/// Format ahead/behind counts into a compact, colored string.
pub fn format_ahead_behind(ahead: usize, behind: usize) -> String {
    match (ahead, behind) {
        (0, 0) => "up to date".bright_black().to_string(),
        (a, 0) => format!("{} {}", "↑".green(), format!("{} ahead", a).green()),
        (0, b) => format!("{} {}", "↓".red(), format!("{} behind", b).red()),
        (a, b) => format!(
            "{} {} {} {}",
            "↑".green(),
            format!("{} ahead", a).green(),
            "↓".red(),
            format!("{} behind", b).red()
        ),
    }
}

// ─── Stack Tree ───────────────────────────────────────────────────────────────

/// Print a stack tree (older helper, kept for potential reuse).
#[allow(dead_code)]
pub fn print_stack_tree(stack_name: &str, branches: &[(String, bool, Option<String>)]) {
    // branches: (name, is_current, pr_url)
    println!(
        "\n  {} {}",
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
            "  {}── {} {}",
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
            println!("  {}   {}", pipe.bright_black(), "│".bright_black());
        }
    }
    println!();
}

// ─── Commit Entry ─────────────────────────────────────────────────────────────

/// Renderable commit entry used by log and compare output.
pub struct CommitEntry {
    pub hash: String,
    pub subject: String,
    pub author: String,
    pub date: String,
    pub refs: String,
    pub graph_prefix: String,
}

impl CommitEntry {
    /// Render a single commit entry with padding and colored fields.
    pub fn render(&self, max_subject: usize) -> String {
        let mut out = String::new();

        // Graph prefix (git's graph art, colorized)
        if !self.graph_prefix.is_empty() {
            let colored_graph = colorize_graph(&self.graph_prefix);
            write!(out, "{}", colored_graph).ok();
        }

        // Hash
        write!(out, " {} ", color_hash(&self.hash)).ok();

        // Subject (truncated + padded to fixed display width)
        let subject = truncate(&self.subject, max_subject);
        let colored_subject = color_subject(&subject);
        let subject_width = console::measure_text_width(&colored_subject);
        let subject_pad = max_subject.saturating_sub(subject_width);
        write!(out, "{}{}", colored_subject, " ".repeat(subject_pad)).ok();

        // Author (truncated + padded to fixed display width)
        let author_max = 20;
        let author = truncate(&self.author, author_max);
        let colored_author = color_author(&author);
        let author_width = console::measure_text_width(&colored_author);
        let author_pad = author_max.saturating_sub(author_width);
        write!(out, "  {}{}", colored_author, " ".repeat(author_pad)).ok();

        // Date
        write!(out, "  {}", color_date(&self.date)).ok();

        // Refs
        if !self.refs.trim().is_empty() {
            write!(out, " {}", format_refs(&self.refs)).ok();
        }

        out
    }
}

/// Truncate a string to a maximum display width, adding an ellipsis if needed.
fn truncate(s: &str, max: usize) -> String {
    if console::measure_text_width(s) > max {
        let truncated = console::truncate_str(s, max.saturating_sub(1), "");
        format!("{}…", truncated)
    } else {
        s.to_string()
    }
}

/// Colorize ASCII graph output from `git log --graph`.
pub fn colorize_graph(graph: &str) -> String {
    // Colorize git graph lines: * | \ / are the main chars
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
                let color = colors[0];
                result.push_str(&format!("{}{}{}", color, ch, reset));
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

// TODO(ui): Centralize icon characters and allow toggling ASCII-only mode.
// TODO(ui): Add width-aware truncation for multi-byte Unicode characters.
