//! Developer and debugging utilities.
//!
//! These commands expose internal tool state for debugging and development.
//! They are intentionally not documented in the main `--help` output beyond
//! their short description — they are power-user tools, not everyday workflows.

use anyhow::{bail, Context, Result};
use rusqlite::Connection;
use std::process::{Command, Stdio};

use crate::config;
use crate::storage::repos;
use crate::ui;

// ─── DB banner ────────────────────────────────────────────────────────────────

/// Pure-ASCII art banner shown before opening the SQLite shell.
///
/// 21 lines tall; uses only `#`, `+`, `-`, `.` to form a shade gradient.
/// `DB_TIPS` has 26 entries — the 5 that overflow the art are rendered below
/// it, indented to the same horizontal position as the tip column.
///
/// Line 1 carries 17 leading spaces to match line 21 (the art is symmetric).
// Raw string (`r"..."`) is used intentionally: Rust's `\`-continuation would
// strip the 17 leading spaces from the first line, breaking symmetry.
const DB_ART: &str = r"                 ...-+######+-...
             -##+.              .+##-
           #+.                      .+#
         .#.                          .#.
         #+                            +#
         #++                          ++#
         #+ ++-                    -++ +#
         #+    .+#+#++-....-++#+#+.    +#
         #+                            +#
         #+.                          .+#
         #+.#+.                    .+#.+#
         #+   .++#++-..    ..-++#++.   +#
         #+          ........          +#
         #+.                          .+#
         #+++.                      .+++#
         #+   ++#+.            .+#++   +#
         #+         ..-++++-..         +#
         .#.                          .#.
           #+.                      .+#
             -##+.              .+##-
                 ...-+######+-...";

/// Quick-reference tips rendered beside the art — one entry per art line.
const DB_TIPS: &[(&str, &str)] = &[
    (".tables", "list all tables in the database"),
    (".schema <table>", "show a table's CREATE statement"),
    (".headers on", "show column names in results"),
    (".mode box", "unicode border-drawing table view"),
    (".mode column", "fixed-width aligned columns"),
    (".mode csv", "comma-separated value output"),
    (".width 20 50", "set per-column display widths"),
    (".output file.txt", "redirect query output to a file"),
    (".read script.sql", "execute SQL from a file"),
    (".dump", "export entire database as SQL"),
    (".dump <table>", "export a single table as SQL"),
    (".timer on", "show query execution time"),
    (".nullvalue NULL", "display NULL values explicitly"),
    (".separator \",\"", "set a custom field delimiter"),
    (".help", "full dot-command reference"),
    (".quit  /  Ctrl+D", "exit the SQLite shell"),
    ("PRAGMA table_info(<t>)", "list columns and their types"),
    ("PRAGMA integrity_check", "verify database data integrity"),
    ("VACUUM", "compact and defragment database"),
    ("EXPLAIN QUERY PLAN ...", "analyse and debug query plans"),
    (
        "SELECT sqlite_version()",
        "print the current SQLite version",
    ),
];

/// Apply per-character colour shading to a single art line.
///
/// Each ASCII shade character maps to a colour on the same warm gradient:
///
/// | Char | Role        | Colour         |
/// |------|-------------|----------------|
/// | `#`  | full body   | bold yellow    |
/// | `+`  | dark edge   | yellow         |
/// | `-`  | mid fade    | dim yellow     |
/// | `.`  | light halo  | uncoloured (terminal default) |
///
/// Spaces and any other character pass through unchanged so that column
/// alignment is not disturbed.
fn colorize_db_art(line: &str) -> String {
    line.chars()
        .map(|c| match c {
            '#' => format!("\x1b[1;33m{c}\x1b[0m"), // bold yellow
            '+' => format!("\x1b[33m{c}\x1b[0m"),   // yellow
            '-' => format!("\x1b[2;33m{c}\x1b[0m"), // dim yellow
            '.' => format!("\x1b[2;33m{c}\x1b[0m"), // dim yellow
            // '.' renders in the terminal's default colour — no extra dim
            _ => c.to_string(),
        })
        .collect()
}

/// Print the two-column welcome banner: art on the left, SQLite tips on the right.
///
/// The loop runs for `max(art_lines, DB_TIPS)` rows so that all tips are shown
/// even when the art is shorter than the tip list.  When the art runs out its
/// column is filled with spaces so the tip column stays at the same position.
fn print_db_banner(db_path: &std::path::Path) {
    let art_lines: Vec<&str> = DB_ART.lines().collect();

    // All art chars are ASCII (1 column each), so chars().count() is exact.
    let art_width = art_lines
        .iter()
        .map(|l| l.trim_end().chars().count())
        .max()
        .unwrap_or(0);

    // Pad the command column so descriptions start at a fixed position.
    let cmd_width = DB_TIPS.iter().map(|(cmd, _)| cmd.len()).max().unwrap_or(0);

    // Gap between the art column and the tips column.
    const GAP: &str = "    ";

    // Header: database path above the art block.
    ui::print_blank();
    let path_str = db_path.display().to_string();
    ui::print_info(&format!("Database  {}", ui::link_primary_bold(&path_str)));
    ui::print_blank();

    let rows = art_lines.len().max(DB_TIPS.len());

    for i in 0..rows {
        // Build the (possibly blank) art column for this row.
        let art_col = if let Some(raw) = art_lines.get(i) {
            let line = raw.trim_end();
            let vis = line.chars().count();
            let pad = " ".repeat(art_width.saturating_sub(vis));
            format!("{}{}", colorize_db_art(line), pad)
        } else {
            // Art has ended — emit blank space to keep the tip column aligned.
            " ".repeat(art_width)
        };

        if let Some((cmd, desc)) = DB_TIPS.get(i) {
            let cmd_pad = " ".repeat(cmd_width.saturating_sub(cmd.len()));
            ui::print_line(&format!(
                "{}{}{}{cmd_pad}  {}",
                art_col,
                GAP,
                ui::primary_bold(cmd),
                ui::muted(desc),
            ));
        } else {
            ui::print_line(&art_col);
        }
    }

    ui::print_blank();
}

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Open (or print the path of) the internal SQLite database.
///
/// When `path_only` is `true`, prints `~/.config/g/g.db` and returns.
/// Otherwise, launches an interactive `sqlite3` shell so the user can run
/// arbitrary SQL queries against the live database.
///
/// # Errors
///
/// Returns an error if:
/// - The database path cannot be determined.
/// - `sqlite3` is not found in `$PATH` (with install instructions).
/// - The `sqlite3` process cannot be spawned.
pub fn db(path_only: bool) -> Result<()> {
    let db_path = config::db_path().context("Could not determine database path")?;

    if path_only {
        ui::print_line(&db_path.display().to_string());
        return Ok(());
    }

    // Verify the DB file actually exists — it is created on first CLI run.
    if !db_path.exists() {
        bail!(
            "Database not found at {}.\n\
             Run any {} command first to initialise it.",
            ui::primary(&db_path.display().to_string()),
            crate::bin_name()
        );
    }

    // Locate sqlite3 in PATH.
    let sqlite3 = which::which("sqlite3").with_context(|| {
        format!(
            "`sqlite3` not found in PATH.\n\
             Install it first:\n\
             {}  brew install sqlite  (macOS)\n\
             {}  apt install sqlite3  (Debian/Ubuntu)\n\
             {}  dnf install sqlite   (Fedora)",
            "  ", "  ", "  ",
        )
    })?;

    // Print the two-column welcome banner before handing off to sqlite3.
    print_db_banner(&db_path);

    // Launch sqlite3 with the DB path, passing all stdio streams through so
    // the user gets a proper interactive terminal experience.
    let status = Command::new(&sqlite3)
        .arg(&db_path)
        // Enable column headers and a box-drawing table format by default —
        // much more readable than the raw pipe-separated output.
        .args(["-cmd", ".headers on", "-cmd", ".mode box"])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("Failed to launch sqlite3 at {}", sqlite3.display()))?;

    ui::print_blank();
    if status.success() {
        ui::print_info("SQLite shell closed.");
    } else if let Some(code) = status.code() {
        ui::print_info(&format!("sqlite3 exited with code {}.", code));
    }
    ui::print_blank();

    Ok(())
}

/// List all repositories tracked in the internal database.
///
/// Prints every repo path that has ever been seen by the tool, ordered by
/// most recently active, alongside first-seen / last-seen timestamps and the
/// total number of command runs recorded for that repo.
///
/// # Errors
///
/// Returns an error if the database cannot be queried.
pub fn repos(conn: &Connection) -> Result<()> {
    let rows = repos::load_all(conn).context("Failed to load repos from database")?;

    if rows.is_empty() {
        ui::print_blank();
        ui::print_info("No repositories tracked yet.");
        ui::print_tip(&format!(
            "Run any {} command inside a git repo to register it.",
            crate::bin_name()
        ));
        ui::print_blank();
        return Ok(());
    }

    ui::print_blank();
    ui::print_fieldset(&format!("Tracked Repositories ({})", rows.len()));
    ui::print_blank();
    let mut table = ui::Table::new(vec!["ID", "Name", "Path", "First seen", "Last seen"]);

    for row in &rows {
        let first = humanise_dt(row.first_seen);
        let last = humanise_dt(row.last_seen);
        table.add_row(vec![
            ui::muted(&row.id.to_string()),
            ui::success_bold(&row.name),
            ui::muted(&row.path),
            first,
            last,
        ]);
    }

    table.print();
    ui::print_blank();
    Ok(())
}

/// Format a UTC datetime as a human-readable relative string.
fn humanise_dt(dt: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(dt);

    let relative = if diff.num_days() > 365 {
        format!("{} years ago", diff.num_days() / 365)
    } else if diff.num_days() > 30 {
        format!("{} months ago", diff.num_days() / 30)
    } else if diff.num_days() > 0 {
        format!("{} days ago", diff.num_days())
    } else if diff.num_hours() > 0 {
        format!("{} hours ago", diff.num_hours())
    } else {
        format!("{} min ago", diff.num_minutes().max(1))
    };
    ui::muted(&relative)
}
