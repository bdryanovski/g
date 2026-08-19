//! Config command implementation.
//!
//! Handles `g config` subcommands: edit, path, get, set, list, menu, themes.

use anyhow::{Context, Result};
use std::io::IsTerminal;

use crate::cli::ConfigArgs;
use crate::config;
use crate::ui;
use crate::{bin_name, APP_ID};

/// Dispatch the `g config` command based on provided arguments.
pub fn dispatch(args: ConfigArgs) -> Result<()> {
    if args.edit {
        return handle_edit();
    }

    if args.path {
        return handle_path();
    }

    if args.themes {
        return handle_themes();
    }

    if args.new_theme {
        return create_theme_wizard(None);
    }

    if let Some(crate::cli::ConfigCmd::Set { key, value }) = &args.cmd {
        return handle_set(key, value);
    }

    if let Some(key) = &args.get {
        return handle_get(key);
    }

    if args.list {
        return handle_list();
    }

    if args.menu {
        return handle_menu();
    }

    if let Some(key) = &args.key {
        return handle_key_lookup(key);
    }

    // Default: show config summary
    handle_summary()
}

// ─── Subcommand handlers ─────────────────────────────────────────────────────

fn handle_edit() -> Result<()> {
    let path = config::config_path()?;
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".into());
    let path_str = path
        .to_str()
        .context("Config path contains non-UTF-8 characters")?;
    std::process::Command::new(&editor)
        .arg(path_str)
        .status()
        .with_context(|| format!("Failed to open editor '{}'", editor))?;
    Ok(())
}

fn handle_path() -> Result<()> {
    let path = config::config_path()?;
    ui::print_line(&path.display().to_string());
    Ok(())
}

fn handle_key_lookup(key: &str) -> Result<()> {
    let cfg = config::load()?;
    let raw = toml::to_string_pretty(&cfg).unwrap_or_default();
    let key_lower = key.to_lowercase();
    let mut found = false;
    for line in raw.lines() {
        if line.to_lowercase().contains(&key_lower) {
            ui::print_line(&ui::paint_text(line));
            found = true;
        }
    }
    if !found {
        ui::print_warning(&format!("Key '{}' not found in config.", key));
    }
    Ok(())
}

fn handle_summary() -> Result<()> {
    let path = config::config_path()?;
    let cfg = config::load()?;
    let db_path = config::db_path()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    ui::print_blank();
    ui::print_fieldset("Configuration");
    ui::print_blank();
    ui::print_key_value_pairs(&[
        (
            "Config file",
            ui::link_primary_bold(&path.display().to_string()),
        ),
        ("Database", ui::link_muted(&db_path)),
    ]);

    ui::print_blank();
    ui::print_fieldset("General");
    ui::print_blank();
    ui::print_key_value_pairs(&[
        ("default_branch", ui::success(&cfg.general.default_branch)),
        (
            "auto_fetch",
            ui::paint_text(&cfg.general.auto_fetch.to_string()),
        ),
        (
            "pager",
            ui::muted(cfg.general.pager.as_deref().unwrap_or("(auto)")),
        ),
    ]);

    ui::print_blank();
    ui::print_fieldset("UI");
    ui::print_blank();
    ui::print_key_value_pairs(&[
        ("theme", ui::paint_text(&cfg.ui.theme)),
        ("colors", ui::paint_text(&cfg.ui.colors.to_string())),
        ("icons", ui::paint_text(&cfg.ui.icons.to_string())),
        ("date_format", ui::paint_text(&cfg.ui.date_format)),
        ("log_limit", ui::paint_text(&cfg.ui.log_limit.to_string())),
        ("show_graph", ui::paint_text(&cfg.ui.show_graph.to_string())),
        ("commit_mode", ui::paint_text(&cfg.ui.commit_mode)),
    ]);

    ui::print_blank();
    ui::print_fieldset("Commit");
    ui::print_blank();
    ui::print_key_value_pairs(&[
        (
            "require_scope",
            ui::paint_text(&cfg.commit.require_scope.to_string()),
        ),
        (
            "require_body",
            ui::paint_text(&cfg.commit.require_body.to_string()),
        ),
        (
            "max_subject",
            ui::paint_text(&cfg.commit.max_subject_length.to_string()),
        ),
        ("sign_off", ui::paint_text(&cfg.commit.sign_off.to_string())),
        ("gpg_sign", ui::paint_text(&cfg.commit.gpg_sign.to_string())),
        ("emoji", ui::paint_text(&cfg.commit.emoji.to_string())),
        ("types", ui::muted(&cfg.commit.types.join(", "))),
    ]);

    ui::print_blank();
    ui::print_fieldset("Diff");
    ui::print_blank();
    ui::print_blank();
    ui::print_fieldset("GitHub");
    ui::print_blank();
    ui::print_key_value_pairs(&[
        ("api_base", ui::paint_text(&cfg.github.api_base)),
        (
            "token",
            if cfg.github.token.is_some() {
                ui::success("*** (set)")
            } else {
                ui::muted("(not set)")
            },
        ),
    ]);

    ui::print_blank();
    ui::print_tip(&format!("{} config --edit  to open in $EDITOR", bin_name()));
    Ok(())
}

// ─── get / set / list / menu ─────────────────────────────────────────────────

fn handle_get(key: &str) -> Result<()> {
    if config::settings::find(key).is_none() {
        ui::print_warning(&format!(
            "Unknown key '{key}' (see `{} config --list`).",
            bin_name()
        ));
        std::process::exit(1);
    }
    match config::settings::get(key)? {
        Some(v) => {
            ui::print_line(&v);
            Ok(())
        }
        None => {
            ui::print_warning(&format!("Key '{key}' is not set in the config file."));
            std::process::exit(1);
        }
    }
}

fn handle_set(key: &str, value: &str) -> Result<()> {
    config::settings::set(key, value)?;
    ui::print_blank();
    ui::print_success(&format!(
        "{} = {}",
        ui::primary_bold(key),
        ui::warning(value)
    ));
    ui::print_blank();
    ui::print_tip(&format!("{} config --get {}  to confirm", bin_name(), key));
    Ok(())
}

fn handle_list() -> Result<()> {
    ui::print_blank();
    ui::print_fieldset("Editable settings");
    ui::print_blank();

    let max_key = config::settings::SCHEMA
        .iter()
        .map(|s| s.key.len())
        .max()
        .unwrap_or(0);

    for s in config::settings::SCHEMA {
        let current = config::settings::get(s.key)
            .ok()
            .flatten()
            .unwrap_or_else(|| "(unset)".to_string());
        let pad = " ".repeat(max_key.saturating_sub(s.key.len()));
        ui::print_line(&format!(
            "  {}{}  {}  {}",
            ui::primary(s.key),
            pad,
            ui::warning(&current),
            ui::muted(s.help),
        ));
    }
    ui::print_blank();
    ui::print_tip(&format!(
        "{} config set <key> <value>  to change one",
        bin_name()
    ));
    Ok(())
}

fn handle_menu() -> Result<()> {
    if ui::is_no_interactive() || !std::io::stdin().is_terminal() {
        return handle_list();
    }

    let entries: Vec<(&'static config::settings::Setting, String)> = config::settings::SCHEMA
        .iter()
        .map(|s| {
            let current = config::settings::get(s.key)
                .ok()
                .flatten()
                .unwrap_or_else(|| "(unset)".to_string());
            (s, current)
        })
        .collect();

    let options: Vec<ui::SelectOption> = entries
        .iter()
        .map(|(s, current)| {
            ui::SelectOption::with_description(s.key.to_string(), format!("= {current}"))
        })
        .collect();

    let Some(idx) = ui::select("Select a setting to change", &options) else {
        return Ok(());
    };
    let (setting, current) = &entries[idx];

    let new_value: Option<String> = match setting.kind {
        config::settings::Kind::Bool => {
            let yes = ui::confirm(
                &format!("{}  (current: {})", setting.key, current),
                current == "true",
            );
            Some(yes.to_string())
        }
        config::settings::Kind::Enum(choices) => {
            let opts: Vec<ui::SelectOption> = choices
                .iter()
                .map(|c| ui::SelectOption::new((*c).to_string()))
                .collect();
            ui::select(&format!("{}  (current: {})", setting.key, current), &opts)
                .map(|i| choices[i].to_string())
        }
        config::settings::Kind::Int | config::settings::Kind::Str => {
            ui::input(&format!("{}  (current: {})", setting.key, current), None)
        }
    };

    let Some(v) = new_value else {
        ui::print_info("Cancelled.");
        return Ok(());
    };

    if v == *current {
        ui::print_info("Value unchanged.");
        return Ok(());
    }

    config::settings::set(setting.key, &v)?;
    ui::print_blank();
    ui::print_success(&format!(
        "{} = {}",
        ui::primary_bold(setting.key),
        ui::warning(&v)
    ));
    ui::print_blank();
    Ok(())
}

// ─── Themes ──────────────────────────────────────────────────────────────────

/// A theme available for selection.
struct ThemeChoice {
    name: String,
    builtin: bool,
}

/// Gather every selectable theme: built-ins first, then custom themes.
fn gather_themes() -> Vec<ThemeChoice> {
    let builtins = ui::theme::builtin_names();
    let mut out: Vec<ThemeChoice> = builtins
        .iter()
        .map(|n| ThemeChoice {
            name: (*n).to_string(),
            builtin: true,
        })
        .collect();

    if let Some(dir) = ui::theme::themes_dir() {
        let mut customs: Vec<String> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("toml") {
                    p.file_stem().and_then(|s| s.to_str()).map(String::from)
                } else {
                    None
                }
            })
            .filter(|n| !builtins.contains(&n.as_str()))
            .collect();
        customs.sort();
        out.extend(customs.into_iter().map(|name| ThemeChoice {
            name,
            builtin: false,
        }));
    }
    out
}

fn handle_themes() -> Result<()> {
    let cfg = config::load().unwrap_or_default();
    let themes = gather_themes();
    if themes.is_empty() {
        ui::print_warning("No themes found.");
        return Ok(());
    }

    let interactive = !ui::is_no_interactive() && std::io::stdin().is_terminal();

    if interactive {
        let current_idx = themes.iter().position(|t| t.name == cfg.ui.theme);
        let mut options: Vec<ui::SelectOption> = themes
            .iter()
            .map(|t| {
                let mut desc = if t.builtin { "built-in" } else { "custom" }.to_string();
                if t.name == cfg.ui.theme {
                    desc.push_str(" · current");
                }
                ui::SelectOption::with_description(&t.name, desc)
            })
            .collect();
        let create_idx = options.len();
        options.push(ui::SelectOption::with_description(
            "+ Create new theme…",
            "wizard: pick a base, override colours, write a new TOML",
        ));

        let prompt = match current_idx {
            Some(_) => format!("Select a theme (current: {})", cfg.ui.theme),
            None => "Select a theme".to_string(),
        };

        if let Some(idx) = ui::select(&prompt, &options) {
            if idx == create_idx {
                return create_theme_wizard(None);
            }
            let chosen = &themes[idx].name;
            if *chosen == cfg.ui.theme {
                ui::print_blank();
                ui::print_info(&format!("Theme unchanged ({chosen})."));
            } else {
                config::set_theme(chosen)?;
                ui::print_blank();
                ui::print_success(&format!("Theme set to '{chosen}'."));
            }
            ui::print_blank();
            ui::print_tip("the new theme applies to your next command");
            return Ok(());
        }
        return Ok(());
    }

    // Non-interactive: print the list
    ui::print_blank();
    ui::print_fieldset("Themes");
    ui::print_blank();
    for t in &themes {
        let marker = if t.name == cfg.ui.theme { ">" } else { " " };
        let kind = if t.builtin { "built-in" } else { "custom" };
        ui::print_line(&format!(
            "  {} {}  {}",
            ui::primary_bold(marker),
            t.name,
            ui::muted(&format!("({kind})"))
        ));
    }
    ui::print_blank();
    ui::print_tip(&format!(
        "{} config --themes  in a terminal to pick interactively",
        bin_name()
    ));
    Ok(())
}

// ─── Theme wizard ────────────────────────────────────────────────────────────

const PALETTE_ROLES: &[(&str, &str)] = &[
    ("primary", "info icon, spinner, active branch"),
    ("success", "checkmarks, added lines, current branch"),
    ("warning", "warnings, commit hashes, staged changes"),
    ("danger", "errors, deleted lines, remote refs"),
    ("muted", "dates, dividers, dim text"),
    ("text", "general body text"),
    ("accent", "section titles, tags, special refs"),
];

fn create_theme_wizard(name: Option<&str>) -> Result<()> {
    if ui::is_no_interactive() || !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "Theme creation requires an interactive terminal. \
             Re-run without --no-interactive."
        );
    }

    ui::print_blank();
    ui::print_fieldset("Create new theme");
    ui::print_blank();

    // 1. Base theme
    let bases = gather_themes();
    let base_options: Vec<ui::SelectOption> = bases
        .iter()
        .map(|t| {
            ui::SelectOption::with_description(
                t.name.clone(),
                if t.builtin { "built-in" } else { "custom" },
            )
        })
        .collect();
    let Some(base_idx) = ui::select("Base theme to extend", &base_options) else {
        ui::print_info("Cancelled.");
        return Ok(());
    };
    let base = &bases[base_idx].name;

    // 2. Name
    let dir = ui::theme::themes_dir()
        .ok_or_else(|| anyhow::anyhow!("could not determine themes directory"))?;
    let name = match name {
        Some(n) => n.to_string(),
        None => {
            let Some(n) = ui::input_validated("Name for the new theme", None, |raw| {
                let v = raw.trim();
                if v.is_empty() {
                    return Err("Name cannot be empty".into());
                }
                if v.contains('/') || v.ends_with(".toml") {
                    return Err("Name should be a plain identifier (no slashes, no .toml)".into());
                }
                Ok(())
            }) else {
                ui::print_info("Cancelled.");
                return Ok(());
            };
            n.trim().to_string()
        }
    };

    let target = dir.join(format!("{name}.toml"));
    if target.exists() {
        anyhow::bail!(
            "Theme file '{}' already exists. Pick a different name or delete it first.",
            target.display()
        );
    }

    // 3. Palette overrides
    ui::print_blank();
    ui::print_info(
        "For each role: press Enter to inherit from the base, or type a colour \
         (hex like #88c0d0, an ANSI name like brightcyan, or a 256-colour index).",
    );
    let mut overrides: Vec<(&'static str, String)> = Vec::new();
    for (role, hint) in PALETTE_ROLES {
        let role_owned = role.to_string();
        let Some(input) = ui::input_validated(&format!("{role}  ({hint})"), None, move |raw| {
            let v = raw.trim();
            if v.is_empty() {
                return Ok(());
            }
            ui::theme::parse_color(v)
                .map(|_| ())
                .map_err(|e| format!("{role_owned}: {e}"))
        }) else {
            ui::print_info("Cancelled.");
            return Ok(());
        };
        let trimmed = input.trim();
        if !trimmed.is_empty() {
            overrides.push((role, trimmed.to_string()));
        }
    }

    // 4. Border style
    let border = pick_or_inherit(
        "Border style",
        &["sharp", "rounded", "heavy", "double", "ascii"],
    )?;

    // 5. Density
    let density = pick_or_inherit("Density", &["normal", "compact", "relaxed"])?;

    // 6. Write the file
    let mut body = String::new();
    body.push_str(&format!("# {} — custom theme: {name}\n", APP_ID));
    body.push_str(&format!(
        "# Created via `{} config --new-theme`.\n",
        bin_name()
    ));
    body.push_str(&format!("name = \"{name}\"\n"));
    body.push_str(&format!("extends = \"{base}\"\n"));
    if let Some(b) = &border {
        body.push_str(&format!("border_style = \"{b}\"\n"));
    }
    if let Some(d) = &density {
        body.push_str(&format!("density = \"{d}\"\n"));
    }
    if !overrides.is_empty() {
        body.push_str("\n[palette]\n");
        for (role, val) in &overrides {
            body.push_str(&format!("{role} = \"{val}\"\n"));
        }
    }

    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create themes directory '{}'", dir.display()))?;
    std::fs::write(&target, body)
        .with_context(|| format!("Failed to write '{}'", target.display()))?;

    ui::print_blank();
    ui::print_success(&format!(
        "Created theme '{}' at {}",
        ui::primary_bold(&name),
        ui::link_muted(&target.display().to_string())
    ));
    ui::print_blank();

    // 7. Offer to activate
    if ui::confirm(&format!("Activate '{}' now?", name), true) {
        config::set_theme(&name)?;
        ui::print_blank();
        ui::print_success(&format!("Theme set to '{}'.", name));
    } else {
        ui::print_tip(&format!(
            "{} config set ui.theme {}  to activate later",
            bin_name(),
            name
        ));
    }
    ui::print_blank();
    Ok(())
}

fn pick_or_inherit(prompt: &str, choices: &[&str]) -> Result<Option<String>> {
    let mut opts: Vec<ui::SelectOption> = choices
        .iter()
        .map(|c| ui::SelectOption::new((*c).to_string()))
        .collect();
    opts.push(ui::SelectOption::with_description(
        "inherit from base",
        "leave the field absent",
    ));
    let inherit_idx = opts.len() - 1;
    match ui::select(prompt, &opts) {
        Some(i) if i == inherit_idx => Ok(None),
        Some(i) => Ok(Some(choices[i].to_string())),
        None => Ok(None),
    }
}
