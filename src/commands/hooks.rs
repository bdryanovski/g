//! `g hooks` command implementations.
//!
//! Manage personal git hooks that coexist with team hooks (like Husky).

use anyhow::{bail, Result};

use crate::cli::HooksCommands;
use crate::commands::Ctx;
use crate::config::{self, hooks::HookType};
use crate::hooks;
use crate::ui::{self, InfoBox};

/// Dispatch a `g hooks` subcommand to its handler.
pub fn dispatch(_ctx: &Ctx, cmd: HooksCommands) -> Result<()> {
    match cmd {
        HooksCommands::List => list(),
        HooksCommands::Run(args) => run(&args.hook),
        HooksCommands::Init => init(),
        HooksCommands::Status => status(),
    }
}

/// `g hooks list` — show all configured hooks.
fn list() -> Result<()> {
    let config = config::load_hooks()?;

    println!();

    if !config.enabled {
        InfoBox::warning("Hooks Disabled")
            .line("Hooks are globally disabled in configuration.")
            .line("")
            .line("Set `enabled = true` in your hooks.toml to enable.")
            .print();
        return Ok(());
    }

    if !config.has_hooks() {
        InfoBox::info("No Hooks Configured")
            .line("No hooks are configured for this repository.")
            .blank()
            .line("Run `g hooks init` to create a template configuration.")
            .print();
        return Ok(());
    }

    // Show configured hooks
    let hook_types = [
        (HookType::PreCommit, &config.pre_commit),
        (HookType::PostCommit, &config.post_commit),
        (HookType::PrePush, &config.pre_push),
        (HookType::PostCheckout, &config.post_checkout),
        (HookType::PostMerge, &config.post_merge),
        (HookType::PreRebase, &config.pre_rebase),
    ];

    let mut lines = Vec::new();
    for (hook_type, hook_config) in &hook_types {
        if let Some(hc) = hook_config {
            if hc.enabled && !hc.commands.is_empty() {
                let status = if hook_type.is_blocking() {
                    ui::danger("●")
                } else {
                    ui::success("●")
                };
                let cmds: Vec<&str> = hc.commands.iter().map(|c| c.display_name()).collect();
                lines.push(format!(
                    "{} {} — {}",
                    status,
                    ui::primary_bold(hook_type.as_str()),
                    ui::muted(&cmds.join(", "))
                ));
            }
        }
    }

    if lines.is_empty() {
        InfoBox::info("No Active Hooks")
            .line("Hooks are configured but all are disabled or empty.")
            .print();
    } else {
        let mut box_ = InfoBox::info("Configured Hooks");
        for line in lines {
            box_ = box_.line(&line);
        }
        box_.blank()
            .line(&format!(
                "{} = blocking (pre-*)  {} = non-blocking (post-*)",
                ui::danger("●"),
                ui::success("●")
            ))
            .print();
    }

    Ok(())
}

/// `g hooks run <hook>` — manually run a specific hook.
fn run(hook_name: &str) -> Result<()> {
    let hook_type = match hook_name {
        "pre-commit" => HookType::PreCommit,
        "post-commit" => HookType::PostCommit,
        "pre-push" => HookType::PrePush,
        "post-checkout" => HookType::PostCheckout,
        "post-merge" => HookType::PostMerge,
        "pre-rebase" => HookType::PreRebase,
        _ => {
            bail!(
                "Unknown hook type: '{}'\n\
                 Available hooks: pre-commit, post-commit, pre-push, post-checkout, post-merge, pre-rebase",
                hook_name
            );
        }
    };

    println!();
    ui::print_info(&format!("Running {} hooks...", hook_name));
    println!();

    match hook_type {
        HookType::PreCommit => hooks::run_pre_commit(false),
        HookType::PostCommit => hooks::run_post_commit(false),
        HookType::PrePush => hooks::run_pre_push(false),
        HookType::PostCheckout => hooks::run_post_checkout(),
        HookType::PostMerge => hooks::run_post_merge(),
        HookType::PreRebase => hooks::run_pre_rebase(false),
    }
}

/// `g hooks init` — create a template hooks.toml.
fn init() -> Result<()> {
    let repo_path = config::repo_hooks_path()?;

    if repo_path.exists() {
        InfoBox::warning("Already Exists")
            .line("Hooks config already exists at:")
            .line(&format!("  {}", repo_path.display()))
            .blank()
            .line("Edit it directly or delete it to regenerate.")
            .print();
        return Ok(());
    }

    let template = r#"# Personal git hooks configuration
# This file is gitignored - it won't affect your team's hooks (like Husky)

# Master switch to enable/disable all hooks
enabled = true

# ─── Pre-commit ───────────────────────────────────────────────────────────────
# Run before `git commit` — failures abort the commit
[pre-commit]
enabled = true
# Only run if staged files match these patterns (empty = always run)
staged_patterns = []
# Pass matching staged files as arguments to commands
pass_files = false

commands = [
    # Example: { run = "cargo fmt -- --check", name = "rustfmt" },
    # Example: { run = "npm run lint", name = "eslint" },
]

# ─── Pre-push ─────────────────────────────────────────────────────────────────
# Run before `git push` — failures abort the push
[pre-push]
enabled = false
commands = [
    # Example: { run = "cargo test", name = "tests" },
]

# ─── Post-commit ──────────────────────────────────────────────────────────────
# Run after `git commit` — failures are warnings only
[post-commit]
enabled = false
commands = []
"#;

    // Create .g directory if needed
    if let Some(parent) = repo_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&repo_path, template)?;

    println!();
    InfoBox::success("Created hooks.toml")
        .line(&format!("Created at: {}", repo_path.display()))
        .blank()
        .line("Edit this file to configure your personal hooks.")
        .line("Add `.g/` to your .gitignore to keep it personal.")
        .print();

    Ok(())
}

/// `g hooks status` — show where hooks config is loaded from.
fn status() -> Result<()> {
    println!();

    // Check each location
    let repo_path = config::repo_hooks_path().ok();
    let hooks_dir = config::hooks_config_dir().ok();

    let mut found = false;

    // 1. Repo-local
    if let Some(ref path) = repo_path {
        if path.exists() {
            InfoBox::success("Hooks Config Found")
                .line("Loading from repo-local config:")
                .line(&format!("  {}", path.display()))
                .print();
            found = true;
        }
    }

    // 2. Per-repo global
    if !found {
        if let Some(ref dir) = hooks_dir {
            if let Ok(repo_name) = crate::commands::git::repo_root() {
                let name = std::path::Path::new(&repo_name)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                let repo_hooks = dir.join(format!("{}.toml", name));
                if repo_hooks.exists() {
                    InfoBox::success("Hooks Config Found")
                        .line("Loading from per-repo global config:")
                        .line(&format!("  {}", repo_hooks.display()))
                        .print();
                    found = true;
                }
            }
        }
    }

    // 3. Default
    if !found {
        if let Some(ref dir) = hooks_dir {
            let default_path = dir.join("default.toml");
            if default_path.exists() {
                InfoBox::success("Hooks Config Found")
                    .line("Loading from default global config:")
                    .line(&format!("  {}", default_path.display()))
                    .print();
                found = true;
            }
        }
    }

    if !found {
        InfoBox::info("No Hooks Config")
            .line("No hooks configuration found.")
            .blank()
            .line("Checked locations (in order):")
            .line(&format!(
                "  1. {}",
                repo_path
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "(not in repo)".to_string())
            ))
            .line(&format!(
                "  2. {}/<repo-name>.toml",
                hooks_dir
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "~/.config/g/hooks".to_string())
            ))
            .line(&format!(
                "  3. {}/default.toml",
                hooks_dir
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "~/.config/g/hooks".to_string())
            ))
            .blank()
            .line("Run `g hooks init` to create a template.")
            .print();
    }

    Ok(())
}
