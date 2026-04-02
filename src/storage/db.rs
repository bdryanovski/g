//! Database connection management.
//!
//! [`open`] is the single entry point for acquiring a [`rusqlite::Connection`].
//! It sets the required PRAGMAs, runs all pending migrations, and optionally
//! imports data from legacy TOML files on the first run with the new binary.

use anyhow::{Context, Result};
use rusqlite::Connection;

use super::migrations;
use crate::config;

/// Open (or create) the SQLite database at `~/.config/g/g.db`.
///
/// On every call this function:
/// 1. Creates the config directory if absent.
/// 2. Opens the database file (creating it if it does not exist).
/// 3. Applies performance and safety PRAGMAs.
/// 4. Runs any pending schema migrations.
/// 5. Checks for legacy TOML files and imports them if present.
///
/// # Errors
///
/// Returns an error if the directory cannot be created, the database cannot be
/// opened, a PRAGMA fails, or a migration fails.
pub fn open() -> Result<Connection> {
    let path = config::db_path()?;

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("Failed to create config directory: {}", dir.display()))?;
    }

    let conn = Connection::open(&path)
        .with_context(|| format!("Failed to open database at {}", path.display()))?;

    apply_pragmas(&conn)?;
    migrations::run(&conn)?;
    migrate_from_toml_if_needed(&conn)?;

    Ok(conn)
}

/// Apply recommended PRAGMAs for a CLI embedded database.
///
/// - `journal_mode=WAL` — allows concurrent readers during a write; also
///   future-proofs the DB for a background daemon writing alongside the CLI.
/// - `synchronous=NORMAL` — fsync only on WAL checkpoints; safe with WAL and
///   significantly faster than the default `FULL`.
/// - `foreign_keys=ON` — enforces FK constraints (disabled by default in SQLite).
/// - `cache_size=-2000` — 2 MB page cache in memory.
fn apply_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA foreign_keys=ON;
         PRAGMA cache_size=-2000;",
    )
    .context("Failed to apply database PRAGMAs")
}

/// Check whether legacy TOML files exist and import them into SQLite.
///
/// This runs once: after a successful import the TOML files are renamed to
/// `.bak` so this function skips them on every subsequent run.
///
/// Import failures are non-fatal — a warning is printed and the TOML file is
/// left in place so the user can retry or recover manually.
fn migrate_from_toml_if_needed(conn: &Connection) -> Result<()> {
    let ws_path = config::workspaces_path()?;
    let st_path = config::stacks_path()?;

    let has_workspaces = ws_path.exists();
    let has_stacks = st_path.exists();

    if !has_workspaces && !has_stacks {
        return Ok(());
    }

    crate::ui::print_info(
        "Detected legacy TOML storage files — migrating to g.db (one-time operation)…",
    );

    if has_workspaces {
        match super::toml_import::import_workspaces(conn, &ws_path) {
            Ok(count) => {
                rename_to_bak(&ws_path);
                crate::ui::print_info(&format!(
                    "  Imported {count} workspace(s) from workspaces.toml"
                ));
            }
            Err(e) => {
                crate::ui::print_warning(&format!(
                    "  Could not import workspaces.toml: {e}. File left in place."
                ));
            }
        }
    }

    if has_stacks {
        match super::toml_import::import_stacks(conn, &st_path) {
            Ok(count) => {
                rename_to_bak(&st_path);
                crate::ui::print_info(&format!("  Imported {count} stack(s) from stacks.toml"));
            }
            Err(e) => {
                crate::ui::print_warning(&format!(
                    "  Could not import stacks.toml: {e}. File left in place."
                ));
            }
        }
    }

    crate::ui::print_info("Migration complete. Backup files kept at *.toml.bak");
    Ok(())
}

/// Rename `path` to `path.bak`, printing a warning on failure.
fn rename_to_bak(path: &std::path::Path) {
    let bak = path.with_extension("toml.bak");
    if let Err(e) = std::fs::rename(path, &bak) {
        crate::ui::print_warning(&format!(
            "  Could not rename {} to {}: {e}",
            path.display(),
            bak.display()
        ));
    }
}
