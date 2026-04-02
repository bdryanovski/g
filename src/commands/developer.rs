//! Developer and debugging utilities.
//!
//! These commands expose internal tool state for debugging and development.
//! They are intentionally not documented in the main `--help` output beyond
//! their short description — they are power-user tools, not everyday workflows.

use anyhow::{bail, Context, Result};
use colored::Colorize;
use rusqlite::Connection;
use std::process::{Command, Stdio};

use crate::config;
use crate::storage::repos;
use crate::ui;

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
        println!("{}", db_path.display());
        return Ok(());
    }

    // Verify the DB file actually exists — it is created on first CLI run.
    if !db_path.exists() {
        bail!(
            "Database not found at {}.\n\
             Run any {} command first to initialise it.",
            db_path.display().to_string().cyan(),
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
            "  ".bright_black(),
            "  ".bright_black(),
            "  ".bright_black(),
        )
    })?;

    println!();
    ui::print_info(&format!(
        "Opening SQLite shell for {}",
        db_path.display().to_string().cyan().underline()
    ));
    println!(
        "  {} .tables          {}",
        "tip:".bright_black(),
        "list all tables".bright_black()
    );
    println!(
        "  {} .schema <table>  {}",
        "    ".bright_black(),
        "show table schema".bright_black()
    );
    println!(
        "  {} .quit            {}",
        "    ".bright_black(),
        "exit the shell".bright_black()
    );
    println!();

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

    println!();
    if status.success() {
        ui::print_info("SQLite shell closed.");
    } else if let Some(code) = status.code() {
        ui::print_info(&format!("sqlite3 exited with code {}.", code));
    }
    println!();

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
        println!();
        println!("  {}", "No repositories tracked yet.".bright_black());
        println!(
            "  {} Run any {} command inside a git repo to register it.",
            "tip:".bright_black(),
            crate::bin_name()
        );
        println!();
        return Ok(());
    }

    println!();
    let mut table = ui::Table::new(vec!["ID", "Name", "Path", "First seen", "Last seen"]);

    for row in &rows {
        // Humanise timestamps to relative strings.
        let first = humanise_dt(row.first_seen);
        let last = humanise_dt(row.last_seen);

        table.add_row(vec![
            row.id.to_string().bright_black().to_string(),
            row.name.green().bold().to_string(),
            row.path.bright_black().to_string(),
            first,
            last,
        ]);
    }

    table.print();
    println!();
    Ok(())
}

/// Format a UTC datetime as a human-readable relative string.
fn humanise_dt(dt: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(dt);

    if diff.num_days() > 365 {
        format!("{} years ago", diff.num_days() / 365)
    } else if diff.num_days() > 30 {
        format!("{} months ago", diff.num_days() / 30)
    } else if diff.num_days() > 0 {
        format!("{} days ago", diff.num_days())
    } else if diff.num_hours() > 0 {
        format!("{} hours ago", diff.num_hours())
    } else {
        format!("{} min ago", diff.num_minutes().max(1))
    }
    .bright_black()
    .to_string()
}
