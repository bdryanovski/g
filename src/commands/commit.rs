//! Interactive commit builder and commit execution.
//!
//! Provides a guided flow for constructing Conventional Commit messages,
//! with optional non-interactive overrides.

use anyhow::{bail, Result};
use colored::Colorize;
use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};

use crate::cli::CommitArgs;
use crate::commands::git as gitcmd;
use crate::config;
use crate::ui;

/// Entry point for `g commit`.
pub fn commit(args: &CommitArgs) -> Result<()> {
    let cfg = config::load()?;

    // Check we're in a repo.
    if !gitcmd::is_inside_git_repo() {
        bail!("Not inside a git repository.");
    }


    // If -a flag, stage everything.
    if args.all {
        gitcmd::git_output(&["add", "-A"])?;
    }

    // Check there's something staged.
    let staged = gitcmd::git_output_lossy(&["diff", "--cached", "--name-only"]);
    if staged.is_empty() && !args.amend {
        ui::print_warning("Nothing staged to commit.");
        println!(
            "  {} Use {} or {} to stage changes.",
            "tip:".bright_black(),
            "git add <file>".yellow(),
            "g commit -a".yellow()
        );
        return Ok(());
    }

    // Show what's staged.
    show_staged_summary()?;

    // Build commit message.
    let message = if let Some(msg) = &args.message {
        // Non-interactive: use provided message
        let body = args.body.clone().unwrap_or_default();
        if body.is_empty() {
            msg.clone()
        } else {
            format!("{}\n\n{}", msg, body)
        }
    } else {
        // Interactive commit builder.
        build_commit_message(args, &cfg)?
    };

    // Validate subject length.
    let subject_line = message.lines().next().unwrap_or(&message);
    if subject_line.len() > cfg.commit.max_subject_length {
        ui::print_warning(&format!(
            "Subject line is {} chars (max {})",
            subject_line.len(),
            cfg.commit.max_subject_length
        ));
        let proceed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Commit anyway?")
            .default(false)
            .interact()?;
        if !proceed {
            return Ok(());
        }
    }

    // Execute the commit.
    let mut git_args = vec!["commit", "-m", &message];

    if args.no_verify {
        git_args.push("--no-verify");
    }

    if args.amend {
        git_args.push("--amend");
    }

    if cfg.commit.gpg_sign {
        git_args.push("-S");
    }

    println!();
    let pb = ui::spinner("Committing…");
    let result = gitcmd::git_output(&git_args);
    pb.finish_and_clear();

    match result {
        Ok(out) => {
            // Parse the commit hash from git output.
            let hash = gitcmd::git_output_lossy(&["rev-parse", "--short", "HEAD"]);
            println!();
            ui::print_success(&format!(
                "{} {}",
                hash.yellow().bold(),
                ui::color_subject(subject_line)
            ));
            if !out.is_empty() {
                println!(
                    "     {}",
                    out.lines().last().unwrap_or("").bright_black()
                );
            }
            println!();
        }
        Err(e) => {
            ui::print_error(&format!("Commit failed: {}", e));
        }
    }

    Ok(())
}

/// Print a short diffstat summary for staged changes.
fn show_staged_summary() -> Result<()> {
    let stat = gitcmd::git_output_lossy(&["diff", "--cached", "--stat"]);
    if stat.is_empty() {
        return Ok(());
    }

    println!();
    println!("  {}", "Staged changes:".bold().white());
    for line in stat.lines().take(12) {
        // Color the stat bar inside the line
        let colored = colorize_stat_line(line);
        println!("     {}", colored);
    }
    if stat.lines().count() > 12 {
        println!("     {}", "…and more".bright_black());
    }
    println!();
    Ok(())
}

fn colorize_stat_line(line: &str) -> String {
    // "  file.rs | 12 +++---"  → color the +++ green and --- red
    if line.contains('|') {
        let parts: Vec<&str> = line.splitn(2, '|').collect();
        let file = parts[0];
        let rest = parts[1];
        let rest_colored = rest
            .replace('+', &"+".green().to_string())
            .replace('-', &"-".red().to_string());
        format!("{}{}{}", file.white(), "|".bright_black(), rest_colored)
    } else {
        // Summary line.
        line.bright_black().to_string()
    }
}

/// Build a commit message using interactive prompts and config rules.
fn build_commit_message(args: &CommitArgs, cfg: &config::Config) -> Result<String> {
    let theme = ColorfulTheme::default();

    println!("  {}", "Building commit message…".bold().cyan());
    println!();

    // Step 1: Type.
    let commit_type = if let Some(t) = &args.r#type {
        t.clone()
    } else {
        let type_descriptions: Vec<String> = cfg
            .commit
            .types
            .iter()
            .map(|t| format_type_label(t, cfg.commit.emoji))
            .collect();

        let idx = Select::with_theme(&theme)
            .with_prompt("  Commit type")
            .items(&type_descriptions)
            .default(0)
            .interact()?;

        cfg.commit.types[idx].clone()
    };

    // Step 2: Scope (optional).
    let scope = if let Some(s) = &args.scope {
        if s.is_empty() { None } else { Some(s.clone()) }
    } else if cfg.commit.require_scope {
        let s: String = Input::with_theme(&theme)
            .with_prompt("  Scope (component/area)")
            .interact_text()?;
        if s.is_empty() { None } else { Some(s) }
    } else {
        let s: String = Input::with_theme(&theme)
            .with_prompt("  Scope (optional, press Enter to skip)")
            .allow_empty(true)
            .interact_text()?;
        if s.is_empty() { None } else { Some(s) }
    };

    // Step 3: Subject.
    let subject: String = Input::with_theme(&theme)
        .with_prompt("  Subject (imperative, present tense)")
        .validate_with(|input: &String| {
            if input.trim().is_empty() {
                Err("Subject cannot be empty")
            } else if input.len() > cfg.commit.max_subject_length {
                Err("Subject is too long")
            } else {
                Ok(())
            }
        })
        .interact_text()?;

    // Build the first line.
    let first_line = if let Some(sc) = &scope {
        format!("{}({}): {}", commit_type, sc, subject.trim())
    } else {
        format!("{}: {}", commit_type, subject.trim())
    };

    // Show preview.
    println!();
    println!("  {} {}", "Preview:".bright_black(), ui::color_subject(&first_line).bold());
    println!();

    // Step 4: Body (optional).
    let body = if cfg.commit.require_body {
        let b: String = Input::with_theme(&theme)
            .with_prompt("  Body (explain WHY, not WHAT)")
            .interact_text()?;
        b
    } else {
        let add_body = Confirm::with_theme(&theme)
            .with_prompt("  Add a body? (explain WHY, motivation, context)")
            .default(false)
            .interact()?;

        if add_body {
            println!("  {} Enter body (empty line to finish, or single dot to skip):", "→".cyan());
            let mut lines = vec![];
            loop {
                let line: String = Input::with_theme(&theme)
                    .with_prompt("  ")
                    .allow_empty(true)
                    .interact_text()?;
                if line == "." || (line.is_empty() && !lines.is_empty()) {
                    break;
                }
                if !line.is_empty() {
                    lines.push(line);
                }
            }
            lines.join("\n")
        } else {
            String::new()
        }
    };

    // Step 5: Footer (breaking change / closes).
    let add_footer = Confirm::with_theme(&theme)
        .with_prompt("  Add footer? (BREAKING CHANGE, closes #issue, etc.)")
        .default(false)
        .interact()?;

    let footer = if add_footer {
        let f: String = Input::with_theme(&theme)
            .with_prompt("  Footer")
            .allow_empty(true)
            .interact_text()?;
        f
    } else {
        String::new()
    };

    // Assemble final message.
    let mut message = first_line.clone();
    if !body.is_empty() {
        message.push_str("\n\n");
        message.push_str(&body);
    }
    if !footer.is_empty() {
        message.push_str("\n\n");
        message.push_str(&footer);
    }

    // Final preview.
    println!();
    println!("  {}", "─".repeat(60).bright_black());
    for line in message.lines() {
        println!("  {}", line.white());
    }
    println!("  {}", "─".repeat(60).bright_black());
    println!();

    let confirm = Confirm::with_theme(&theme)
        .with_prompt("  Commit with this message?")
        .default(true)
        .interact()?;

    if !confirm {
        bail!("Commit cancelled.");
    }

    Ok(message)
}

/// Render a commit type label with icon and description.
fn format_type_label(t: &str, emoji: bool) -> String {
    let (icon, description) = match t {
        "feat"     => ("✨", "A new feature"),
        "fix"      => ("🐛", "A bug fix"),
        "docs"     => ("📝", "Documentation changes"),
        "style"    => ("💅", "Formatting, style changes"),
        "refactor" => ("♻️ ", "Code refactoring"),
        "perf"     => ("⚡", "Performance improvements"),
        "test"     => ("✅", "Adding or fixing tests"),
        "build"    => ("🏗️ ", "Build system changes"),
        "ci"       => ("👷", "CI/CD changes"),
        "chore"    => ("🔧", "Other changes (no src/test)"),
        "revert"   => ("⏪", "Reverting a previous commit"),
        _          => ("·", ""),
    };
    if description.is_empty() {
        if emoji {
            format!("{} {}", icon, t)
        } else {
             t.to_string()
        }
    } else {
        if emoji {
            format!("{} {:12}  {}", icon, t, description.bright_black())
        } else {
            format!("{} {:12}  {}", icon, t, description.bright_black())
        }
    }
}

// TODO(commit): Honor `commit.template` from config when building messages.
// TODO(commit): Add a non-interactive `--edit` flow to open an editor before commit.
// TODO(commit): Improve staging summary to group by file status.
