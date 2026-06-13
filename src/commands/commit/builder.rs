//! Interactive commit-message builders.
//!
//! Two near-identical builders share the same five steps — type, scope,
//! subject, body, footer — but use different prompt mechanisms:
//!
//! - [`build_commit_message_interactive`] uses the full-screen ratatui TUI
//!   (alternate screen).
//! - [`build_commit_message_inline`] uses inline prompts that stay in the
//!   terminal scrollback.
//!
//! Both rely on [`select_commit_type`] for the type picker; that helper itself
//! delegates to `ui::select` / `ui::input`, which respect the global
//! `INLINE_PROMPTS` flag — so the two builders share the same picker
//! implementation and differ only in their `ui::confirm` / `ui::input` modes.

use crate::cli::CommitArgs;
use crate::config;
use crate::ui;
use anyhow::{bail, Result};

use super::preview::type_label_parts;

/// Show the commit-type picker and return the chosen type string.
///
/// Appends an **Other…** option at the end of the configured type list so the
/// user can enter any arbitrary type without editing `config.toml`.  When
/// "Other…" is chosen, a follow-up text prompt is shown.
pub(super) fn select_commit_type(args: &CommitArgs, cfg: &config::Config) -> Result<String> {
    if let Some(t) = &args.r#type {
        return Ok(t.clone());
    }

    // Build the option list from config types + a trailing "other" entry.
    let mut options: Vec<ui::SelectOption> = cfg
        .commit
        .types
        .iter()
        .map(|t| {
            let (_, description) = type_label_parts(t);
            if description.is_empty() {
                ui::SelectOption::new(t.clone())
            } else {
                ui::SelectOption::with_description(t.clone(), description)
            }
        })
        .collect();
    options.push(ui::SelectOption::with_description(
        "other".to_string(),
        "Custom type — enter manually",
    ));

    let idx = ui::select("Type", &options).ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?;

    // Last item = "other" → prompt for a free-form type.
    if idx == options.len() - 1 {
        let custom =
            ui::input("Custom type", None).ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?;
        let custom = custom.trim().to_string();
        if custom.is_empty() {
            anyhow::bail!("Commit cancelled — custom type cannot be empty.");
        }
        Ok(custom)
    } else {
        Ok(cfg.commit.types[idx].clone())
    }
}

/// Build a Conventional Commit message using the full-screen ratatui TUI.
///
/// Used when `[ui] commit_mode = "interactive"` (default).  Each step opens
/// a dedicated TUI screen with a ratatui-cheese Help bar at the bottom.
pub(super) fn build_commit_message_interactive(
    args: &CommitArgs,
    cfg: &config::Config,
) -> Result<String> {
    // Step 1: Type selection (shared helper handles "Other…" too).
    let commit_type = select_commit_type(args, cfg)?;

    // Step 2: Scope.
    let scope = if let Some(s) = &args.scope {
        if s.is_empty() {
            None
        } else {
            Some(s.clone())
        }
    } else {
        let prompt = if cfg.commit.require_scope {
            "Scope (required)"
        } else {
            "Scope (optional, Enter to skip)"
        };
        let s = ui::input(prompt, None).ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?;
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    };

    // Step 3: Subject.
    let max_len = cfg.commit.max_subject_length;
    let subject = ui::input_validated("Subject", None, move |val| {
        if val.trim().is_empty() {
            Err("Subject cannot be empty".to_string())
        } else if val.len() > max_len {
            Err(format!("Subject is too long ({}/{})", val.len(), max_len))
        } else {
            Ok(())
        }
    })
    .ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?;

    let first_line = if let Some(sc) = &scope {
        format!("{}({}): {}", commit_type, sc, subject.trim())
    } else {
        format!("{}: {}", commit_type, subject.trim())
    };

    // Step 4: Body.
    let body = if cfg.commit.require_body {
        ui::input("Body (explain WHY, not WHAT)", None)
            .ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?
    } else if cfg.commit.prompt_body && ui::confirm("Add a body?", false) {
        ui::input("Body", None).ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?
    } else {
        String::new()
    };

    // Step 5: Footer.
    let footer = if cfg.commit.prompt_footer
        && ui::confirm("Add footer? (BREAKING CHANGE, closes #N…)", false)
    {
        ui::input("Footer", None).ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?
    } else {
        String::new()
    };

    // Assemble the full message.
    let mut message = first_line.clone();
    if !body.is_empty() {
        message.push_str("\n\n");
        message.push_str(&body);
    }
    if !footer.is_empty() {
        message.push_str("\n\n");
        message.push_str(&footer);
    }

    // Final preview + confirmation.
    ui::print_blank();
    ui::print_fieldset("Preview");
    ui::print_blank();
    for line in message.lines() {
        ui::print_indented(&ui::paint_text(line));
    }
    ui::print_blank();

    if !ui::confirm("Commit with this message?", true) {
        bail!("Commit cancelled.");
    }

    Ok(message)
}

/// Build a Conventional Commit message using inline (non-fullscreen) prompts.
///
/// Used when `[ui] commit_mode = "inline"`.  Each step prints its prompt and
/// the user's answer into the normal terminal scroll buffer — the commit
/// history is visible after the command completes and no alternate screen is
/// entered or restored.
pub(super) fn build_commit_message_inline(
    args: &CommitArgs,
    cfg: &config::Config,
) -> Result<String> {
    ui::print_blank();

    // ── Step 1: Type (shared helper handles "Other…") ────────────────────────
    let commit_type = select_commit_type(args, cfg)?;

    // ── Step 2: Scope ─────────────────────────────────────────────────────────
    let scope = if let Some(s) = &args.scope {
        if s.is_empty() {
            None
        } else {
            Some(s.clone())
        }
    } else {
        let prompt = if cfg.commit.require_scope {
            "Scope (required)"
        } else {
            "Scope (optional, Enter to skip)"
        };
        let s = ui::input(prompt, None).ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?;
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    };

    // ── Step 3: Subject ───────────────────────────────────────────────────────
    let max_len = cfg.commit.max_subject_length;
    let subject = ui::input_validated("Subject", None, move |val| {
        if val.trim().is_empty() {
            Err("Subject cannot be empty".to_string())
        } else if val.len() > max_len {
            Err(format!("Subject is too long ({}/{})", val.len(), max_len))
        } else {
            Ok(())
        }
    })
    .ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?;

    let first_line = match &scope {
        Some(sc) => format!("{}({}): {}", commit_type, sc, subject.trim()),
        None => format!("{}: {}", commit_type, subject.trim()),
    };

    // ── Step 4: Body ──────────────────────────────────────────────────────────
    let body = if cfg.commit.require_body {
        ui::input("Body (explain WHY, not WHAT)", None)
            .ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?
    } else if cfg.commit.prompt_body && ui::confirm("Add a body?", false) {
        ui::input("Body", None).ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?
    } else {
        String::new()
    };

    // ── Step 5: Footer ────────────────────────────────────────────────────────
    let footer = if cfg.commit.prompt_footer
        && ui::confirm("Add footer? (BREAKING CHANGE, closes #N…)", false)
    {
        ui::input("Footer", None).ok_or_else(|| anyhow::anyhow!("Commit cancelled."))?
    } else {
        String::new()
    };

    // ── Assemble message ──────────────────────────────────────────────────────
    let mut message = first_line.clone();
    if !body.is_empty() {
        message.push_str("\n\n");
        message.push_str(&body);
    }
    if !footer.is_empty() {
        message.push_str("\n\n");
        message.push_str(&footer);
    }

    // ── Preview + confirmation ────────────────────────────────────────────────
    ui::print_blank();
    ui::print_fieldset("Preview");
    ui::print_blank();
    for line in message.lines() {
        ui::print_indented(&ui::paint_text(line));
    }
    ui::print_blank();

    if !ui::confirm("Commit with this message?", true) {
        bail!("Commit cancelled.");
    }

    Ok(message)
}
