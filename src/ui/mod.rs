use colored::Colorize;
use std::fmt::Write as FmtWrite;

// ─── Theme Colors ─────────────────────────────────────────────────────────────

/// Print an info message with a cyan bullet
pub fn print_info(msg: &str) {
    println!("  {} {}", "ℹ".cyan(), msg);
}

/// Print a success message with a green checkmark
pub fn print_success(msg: &str) {
    println!("  {} {}", "✓".green().bold(), msg);
}

/// Print a warning message with a yellow warning sign
pub fn print_warning(msg: &str) {
    eprintln!("  {} {}", "⚠".yellow().bold(), msg);
}

/// Print an error message with a red X
pub fn print_error(msg: &str) {
    eprintln!("  {} {}", "✗".red().bold(), msg);
}

/// Print a section header in a box
#[allow(dead_code)]
pub fn print_header(title: &str) {
    let width = title.len() + 4;
    let line = "─".repeat(width);
    println!("{}", format!("╭{}╮", line).bright_black());
    println!("{} {} {}", "│".bright_black(), title.bold().white(), "│".bright_black());
    println!("{}", format!("╰{}╯", line).bright_black());
}

/// Print a section title (lighter than header)
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
#[allow(dead_code)]
pub fn print_divider() {
    println!("  {}", "─".repeat(60).bright_black());
}

// ─── Git Color Helpers ────────────────────────────────────────────────────────

pub fn color_hash(hash: &str) -> String {
    hash.yellow().dimmed().to_string()
}

pub fn color_branch(name: &str) -> String {
    if name.starts_with("origin/") || name.starts_with("upstream/") {
        name.red().bold().to_string()
    } else if name == "HEAD" {
        name.cyan().bold().to_string()
    } else {
        name.green().bold().to_string()
    }
}

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

pub fn color_author(name: &str) -> String {
    name.cyan().to_string()
}

pub fn color_date(date: &str) -> String {
    date.bright_black().to_string()
}

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
        } else if prefix.starts_with("chore") || prefix.starts_with("build") || prefix.starts_with("ci") {
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

pub fn color_added(n: i64) -> String {
    format!("+{}", n).green().to_string()
}

pub fn color_deleted(n: i64) -> String {
    format!("-{}", n).red().to_string()
}

// ─── Status Icons ─────────────────────────────────────────────────────────────

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

/// Format git ref decorations (HEAD -> main, origin/main) into colored badges
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
                format!("{} {} {}", "HEAD →".cyan().bold(), "".bright_black(), color_branch(branch))
            } else {
                color_ref(r)
            }
        })
        .collect();
    if formatted.is_empty() {
        String::new()
    } else {
        format!(" {} {} {}", "(".bright_black(), formatted.join(&" · ".bright_black().to_string()), ")".bright_black())
    }
}

// ─── Table Formatter ─────────────────────────────────────────────────────────

pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    col_widths: Vec<usize>,
}

impl Table {
    pub fn new(headers: Vec<&str>) -> Self {
        let col_widths = headers.iter().map(|h| h.len()).collect();
        Self {
            headers: headers.into_iter().map(String::from).collect(),
            rows: vec![],
            col_widths,
        }
    }

    pub fn add_row(&mut self, row: Vec<String>) {
        for (i, cell) in row.iter().enumerate() {
            // strip ANSI codes for width calculation
            let visible_len = strip_ansi(cell).len();
            if i < self.col_widths.len() {
                self.col_widths[i] = self.col_widths[i].max(visible_len);
            }
        }
        self.rows.push(row);
    }

    pub fn print(&self) {
        // Header
        let header_cells: Vec<String> = self
            .headers
            .iter()
            .enumerate()
            .map(|(i, h)| format!("{:<width$}", h.bold().bright_white().to_string(), width = self.col_widths[i]))
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
                .map(|(i, cell)| {
                    let visible_len = strip_ansi(cell).len();
                    let padding = self.col_widths.get(i).copied().unwrap_or(0).saturating_sub(visible_len);
                    format!("{}{}", cell, " ".repeat(padding))
                })
                .collect();
            println!("  {}", cells.join("  "));
        }
    }
}

/// Strip ANSI escape codes for length calculation
pub fn strip_ansi(s: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;
    for ch in s.chars() {
        if ch == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if ch == 'm' {
                in_escape = false;
            }
        } else {
            result.push(ch);
        }
    }
    result
}

// ─── Branch Ahead/Behind ─────────────────────────────────────────────────────

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

#[allow(dead_code)]
pub fn print_stack_tree(stack_name: &str, branches: &[(String, bool, Option<String>)]) {
    // branches: (name, is_current, pr_url)
    println!("\n  {} {}", "Stack:".bold().bright_white(), stack_name.cyan().bold());
    println!();
    let last = branches.len().saturating_sub(1);
    for (i, (branch, is_current, pr_url)) in branches.iter().enumerate() {
        let connector = if i == last { "└" } else { "├" };
        let pipe = if i == last { " " } else { "│" };
        let marker = if *is_current { "◉".green().bold().to_string() } else { "◯".bright_black().to_string() };

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

pub struct CommitEntry {
    pub hash: String,
    pub subject: String,
    pub author: String,
    pub date: String,
    pub refs: String,
    pub graph_prefix: String,
}

impl CommitEntry {
    pub fn render(&self, max_subject: usize) -> String {
        let mut out = String::new();

        // Graph prefix (git's graph art, colorized)
        if !self.graph_prefix.is_empty() {
            let colored_graph = colorize_graph(&self.graph_prefix);
            write!(out, "{}", colored_graph).ok();
        }

        // Hash
        write!(out, " {} ", color_hash(&self.hash)).ok();

        // Subject (truncated)
        let subject = if self.subject.len() > max_subject {
            format!("{}…", &self.subject[..max_subject - 1])
        } else {
            self.subject.clone()
        };
        write!(out, "{:<width$}", color_subject(&subject), width = max_subject + 10).ok();

        // Author
        write!(out, "  {:<20}", color_author(&truncate(&self.author, 20))).ok();

        // Date
        write!(out, "  {}", color_date(&self.date)).ok();

        // Refs
        if !self.refs.trim().is_empty() {
            write!(out, "{}", format_refs(&self.refs)).ok();
        }

        out
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max - 1])
    } else {
        s.to_string()
    }
}

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
