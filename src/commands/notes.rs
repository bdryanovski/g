//! `g notes` command dispatch.
//!
//! Surfaces list / show / edit / delete / clear operations over private review
//! notes stored in the local SQLite DB.  Notes are written from inside the
//! `g diff` TUI via the `c` key; this module is the non-TUI way to manage
//! them.
//!
//! The "public" bucket of the review-comment model — comments pushed to a
//! GitHub PR — is not touched here.  Those go straight through `github` and
//! are not persisted locally.

use anyhow::{anyhow, bail, Context, Result};

use crate::cli::NotesCommands;
use crate::commands::Ctx;
use crate::storage;
use crate::ui;

/// Top-level dispatcher invoked from `main::run()`.
pub fn dispatch(ctx: &Ctx<'_>, cmd: NotesCommands) -> Result<()> {
    let conn = ctx.conn;
    let repo_id = ctx.repo_id.ok_or_else(|| {
        anyhow!("not inside a git repo — `g notes` needs a repo to anchor notes")
    })?;

    match cmd {
        NotesCommands::List => list(conn, repo_id),
        NotesCommands::Show { id } => show(conn, id),
        NotesCommands::Edit { id } => edit(conn, id),
        NotesCommands::Delete { id } => delete(conn, id),
        NotesCommands::Clear { path, force } => clear(conn, repo_id, path, force),
        NotesCommands::Publish { id } => publish(conn, id),
    }
}

/// `g notes list` — print every note in the current repo, newest first.
fn list(conn: &rusqlite::Connection, repo_id: i64) -> Result<()> {
    let notes = storage::reviews::load_for_repo(conn, repo_id)
        .context("Failed to load review notes")?;
    if notes.is_empty() {
        ui::print_line("No private review notes in this repo.");
        return Ok(());
    }
    ui::print_line(&format!(
        "  {} private review note(s) in this repo\n",
        notes.len()
    ));
    for n in notes {
        let side = match n.line_kind {
            storage::LineKind::New => "+",
            storage::LineKind::Old => "-",
        };
        let preview: String = n
            .body
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(60)
            .collect();
        ui::print_line(&format!(
            "  {:>4}  {}{}{}  {}",
            n.id, n.path, side, n.line, preview
        ));
    }
    Ok(())
}

/// `g notes show <id>` — print the body and anchor of a single note.
fn show(conn: &rusqlite::Connection, id: i64) -> Result<()> {
    let Some(note) = storage::reviews::load_by_id(conn, id)
        .context("Failed to load note")?
    else {
        bail!("Note {} not found", id);
    };
    let side = match note.line_kind {
        storage::LineKind::New => "+",
        storage::LineKind::Old => "-",
    };
    let sha = note.commit_hash.unwrap_or_else(|| "(unstaged)".to_string());
    ui::print_line(&format!(
        "  note {}  repo={}  {}{}{}  @{}",
        note.id, note.repo_id, note.path, side, note.line, sha
    ));
    ui::print_line(&format!(
        "  author: {}  created: {}  updated: {}",
        note.author.unwrap_or_default(),
        note.created_at.format("%Y-%m-%d %H:%M UTC"),
        note.updated_at.format("%Y-%m-%d %H:%M UTC"),
    ));
    ui::print_line("");
    for line in note.body.lines() {
        ui::print_line(&format!("  {}", line));
    }
    Ok(())
}

/// `g notes edit <id>` — open `$EDITOR` on the body, then persist the new text.
fn edit(conn: &rusqlite::Connection, id: i64) -> Result<()> {
    let Some(note) = storage::reviews::load_by_id(conn, id)
        .context("Failed to load note")?
    else {
        bail!("Note {} not found", id);
    };
    let new_body = open_editor(&note.body)?;
    if new_body == note.body {
        ui::print_line(&format!("Note {} unchanged.", id));
        return Ok(());
    }
    storage::reviews::update_body(conn, id, &new_body)
        .context("Failed to persist edited note body")?;
    ui::print_line(&format!("Saved note {}", id));
    Ok(())
}

/// Spawn `$EDITOR` (or `vi` as a fallback) on `initial_text` and return the
/// editor's saved content.  Uses a tempfile so editors that write in-place
/// (the common case) work without any extra plumbing.
fn open_editor(initial_text: &str) -> Result<String> {
    use std::io::Write;
    let mut tmp = tempfile::Builder::new()
        .prefix("g-note-")
        .suffix(".md")
        .tempfile()
        .context("Failed to create tempfile for editor")?;
    tmp.write_all(initial_text.as_bytes())
        .context("Failed to seed editor tempfile")?;
    tmp.flush().context("Failed to flush editor tempfile")?;

    let path = tmp.path().to_path_buf();
    // Keep the temp file alive until after the editor exits; drop the close
    // handle here so we can re-read the path afterwards.
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let status = std::process::Command::new(&editor)
        .arg(&path)
        .status()
        .with_context(|| format!("Failed to spawn $EDITOR ({})", editor))?;
    if !status.success() {
        bail!("$EDITOR ({}) exited with non-zero status", editor);
    }
    let updated = std::fs::read_to_string(&path).context("Failed to re-read editor tempfile")?;
    drop(tmp); // explicit drop for clarity; would happen at scope end anyway.
    Ok(updated)
}

/// `g notes delete <id>` — drop a single note.
fn delete(conn: &rusqlite::Connection, id: i64) -> Result<()> {
    storage::reviews::delete(conn, id).context("Failed to delete note")?;
    ui::print_line(&format!("Deleted note {}", id));
    Ok(())
}

/// `g notes clear [--force] [<path>]` — drop notes for a file (or all notes).
fn clear(
    conn: &rusqlite::Connection,
    repo_id: i64,
    path: Option<String>,
    force: bool,
) -> Result<()> {
    if !force {
        bail!("Refusing to clear notes without --force.  Re-run with --force to confirm.");
    }
    match path {
        Some(p) => {
            let n = storage::reviews::delete_for_file(conn, repo_id, &p)
                .context("Failed to clear notes for file")?;
            ui::print_line(&format!("Cleared {} note(s) for {}", n, p));
        }
        None => {
            let n = storage::reviews::delete_all_for_repo(conn, repo_id)
                .context("Failed to clear all notes")?;
            ui::print_line(&format!("Cleared {} note(s)", n));
        }
    }
    Ok(())
}

/// `g notes publish <id>` — move a saved private note to the public GitHub PR
/// bucket by posting it as a line-anchored review comment.
///
/// Looks up the open PR whose head matches the current branch, then invokes
/// `github::create_pr_review_comment` with the note's anchor.  The local row
/// is **not** deleted — the user can `g notes delete <id>` after confirming
/// the post landed on GitHub.
fn publish(conn: &rusqlite::Connection, id: i64) -> Result<()> {
    let Some(note) = storage::reviews::load_by_id(conn, id)
        .context("Failed to load note")?
    else {
        bail!("Note {} not found", id);
    };
    let Some(commit_hash) = note.commit_hash.as_deref() else {
        bail!(
            "Note {} has no commit attribution (unstaged diff).  Re-anchor it \
             by re-running `g diff` against a committed ref before publishing.",
            id
        );
    };

    let cfg = crate::config::load().unwrap_or_default();
    let token = std::env::var("GITHUB_TOKEN")
        .ok()
        .or(cfg.github.token.clone())
        .ok_or_else(|| {
            anyhow!(
                "GITHUB_TOKEN env var or [github].token config is required to \
                 publish a review comment"
            )
        })?;
    let api_base = cfg.github.api_base.clone();

    let Some((owner, repo, pr_number, _head_sha)) =
        crate::github::detect_pr_for_current_branch(&token, &api_base)?
    else {
        bail!(
            "Current branch is not the head of an open PR.  Run `g stack pr` \
             (or open a PR on github.com) to publish the comment there."
        );
    };

    let side = match note.line_kind {
        storage::LineKind::New => crate::diff::reviews::CommentSide::New,
        storage::LineKind::Old => crate::diff::reviews::CommentSide::Old,
    };

    let posted = crate::github::create_pr_review_comment(
        &token,
        &api_base,
        &owner,
        &repo,
        pr_number,
        &note.path,
        note.line,
        side,
        commit_hash,
        &note.body,
    )
    .context("Failed to post review comment to GitHub")?;

    ui::print_line(&format!(
        "Published note {} (gh comment #{}) → {}",
        id, posted.id, posted.html_url
    ));
    Ok(())
}