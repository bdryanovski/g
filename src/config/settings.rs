//! Schema-driven, comment-preserving access to individual `config.toml`
//! settings.
//!
//! This powers `g config <key>`, `g config <key> <value>`, `g config --list`
//! and the interactive `g config --menu`.  All writes go through
//! [`toml_edit::DocumentMut`], so the file keeps its comments, ordering and
//! formatting — it stays just as easy to hand-edit afterwards.
//!
//! A small static [`schema`] describes every editable scalar key, its type and
//! a one-line help string.  The schema is the single source of truth for both
//! validation (CLI) and the interactive picker (menu).

use anyhow::{anyhow, bail, Context, Result};
use std::fs;
use toml_edit::{DocumentMut, Item, Table, TableLike, Value};

use super::{config_path, default_config_toml};

/// The type of a setting — drives value parsing and how the menu prompts.
#[derive(Debug, Clone, Copy)]
pub enum SettingKind {
    /// Free-form string.
    Str,
    /// Boolean (`true` / `false`).
    Bool,
    /// Signed integer.
    Int,
    /// One of a fixed set of string choices.
    Enum(&'static [&'static str]),
}

/// One editable configuration key.
#[derive(Debug, Clone, Copy)]
pub struct Setting {
    /// Dotted path into the TOML document, e.g. `"ui.log_limit"`.
    pub key: &'static str,
    /// Value type / allowed choices.
    pub kind: SettingKind,
    /// One-line description shown in `--list` and the menu.
    pub help: &'static str,
}

/// Every editable scalar setting, grouped roughly by section.
///
/// Only simple scalar keys are exposed here; structured keys (aliases, the
/// `commit.types` array, plugin paths) remain hand-edited in the file.
pub fn schema() -> &'static [Setting] {
    use SettingKind::*;
    const DATE_FORMATS: &[&str] = &["relative", "short", "iso", "rfc"];
    const MODES: &[&str] = &["interactive", "inline"];
    const BORDERS: &[&str] = &["sharp", "rounded", "heavy", "double", "ascii"];
    const DENSITY: &[&str] = &["normal", "compact", "relaxed"];

    &[
        // ── general ──
        Setting { key: "general.default_branch", kind: Str, help: "Base branch for comparisons and new stacks" },
        Setting { key: "general.auto_fetch", kind: Bool, help: "Run `git fetch` before branch comparisons" },
        // ── ui ──
        Setting { key: "ui.theme", kind: Str, help: "Color theme (built-in name, custom name, or path)" },
        Setting { key: "ui.colors", kind: Bool, help: "Enable ANSI-colored output" },
        Setting { key: "ui.icons", kind: Bool, help: "Use Unicode icons (false = ASCII)" },
        Setting { key: "ui.date_format", kind: Enum(DATE_FORMATS), help: "Date display format" },
        Setting { key: "ui.log_limit", kind: Int, help: "Default number of commits shown by `g log`" },
        Setting { key: "ui.show_graph", kind: Bool, help: "Show the branch graph in `g log`" },
        Setting { key: "ui.commit_mode", kind: Enum(MODES), help: "Prompt mode for the commit builder" },
        Setting { key: "ui.prompt_mode", kind: Enum(MODES), help: "Global prompt rendering mode" },
        Setting { key: "ui.border_style", kind: Enum(BORDERS), help: "Box-drawing style for rules/tables/trees" },
        Setting { key: "ui.density", kind: Enum(DENSITY), help: "Layout spacing / indentation" },
        // ── commit ──
        Setting { key: "commit.require_scope", kind: Bool, help: "Require a scope in commit messages" },
        Setting { key: "commit.require_body", kind: Bool, help: "Require a body in commit messages" },
        Setting { key: "commit.prompt_body", kind: Bool, help: "Prompt to add a body during `g commit`" },
        Setting { key: "commit.prompt_footer", kind: Bool, help: "Prompt to add a footer during `g commit`" },
        Setting { key: "commit.max_subject_length", kind: Int, help: "Warn above this subject length" },
        Setting { key: "commit.sign_off", kind: Bool, help: "Append a Signed-off-by trailer (-s)" },
        Setting { key: "commit.gpg_sign", kind: Bool, help: "GPG-sign commits (-S)" },
        Setting { key: "commit.emoji", kind: Bool, help: "Show emoji in the commit type picker" },
        // ── diff ──
        Setting { key: "diff.tool", kind: Str, help: "Diff tool: auto | builtin | delta | <path>" },
        Setting { key: "diff.context_lines", kind: Int, help: "Context lines around each diff hunk" },
        // ── github ──
        Setting { key: "github.api_base", kind: Str, help: "GitHub API base URL (for Enterprise)" },
        // ── workspace ──
        Setting { key: "workspace.separator", kind: Str, help: "Separator in sibling worktree directory names" },
        // ── stage ──
        Setting { key: "stage.confirm_revert", kind: Bool, help: "Confirm before reverting a file in `g stage`" },
        // ── plugins ──
        Setting { key: "plugins.discover", kind: Bool, help: "Discover `g-*` executables on PATH" },
    ]
}

/// Find the schema entry for `key` (case-insensitive, exact match).
pub fn find_setting(key: &str) -> Option<&'static Setting> {
    schema().iter().find(|s| s.key.eq_ignore_ascii_case(key.trim()))
}

/// Load the config file (or the built-in default template) as an editable,
/// comment-preserving document.
fn load_document() -> Result<DocumentMut> {
    let path = config_path()?;
    let raw = if path.exists() {
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?
    } else {
        default_config_toml().to_string()
    };
    raw.parse::<DocumentMut>().context("Failed to parse config TOML")
}

/// Return the current value of `key` rendered as a display string, or `None`
/// when the key is not present in the file.
pub fn get_value(key: &str) -> Result<Option<String>> {
    let doc = load_document()?;
    Ok(nav(&doc, key).and_then(render_value))
}

/// Validate, type-coerce and persist `key = value`, preserving comments and
/// formatting.  The known schema entry determines how `raw` is parsed.
///
/// # Errors
///
/// Returns an error when the key is unknown, the value is invalid for the
/// setting's type, or the file cannot be read/written.
pub fn set_value(key: &str, raw: &str) -> Result<()> {
    let setting = find_setting(key)
        .ok_or_else(|| anyhow!("unknown config key '{}' (see `g config --list`)", key))?;
    let new_value = coerce(&setting.kind, raw)?;

    let mut doc = load_document()?;
    let parts: Vec<&str> = setting.key.split('.').collect();
    let (sections, last) = parts.split_at(parts.len() - 1);
    let last = last[0];

    let table = table_path(&mut doc, sections)?;

    // Preserve the existing line's formatting/comment where possible.
    let mut value = new_value;
    if let Some(existing) = table.get(last).and_then(Item::as_value) {
        *value.decor_mut() = existing.decor().clone();
    } else {
        value.decor_mut().set_prefix(" ");
    }
    table.insert(last, Item::Value(value));

    let path = config_path()?;
    fs::write(&path, doc.to_string())
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Navigate a dotted path to an [`Item`].
fn nav<'a>(doc: &'a DocumentMut, key: &str) -> Option<&'a Item> {
    let mut parts = key.trim().split('.');
    let mut item = doc.as_table().get(parts.next()?)?;
    for p in parts {
        item = item.as_table_like()?.get(p)?;
    }
    Some(item)
}

/// Render a scalar [`Item`] as a plain display string.
fn render_value(item: &Item) -> Option<String> {
    let v = item.as_value()?;
    Some(match v {
        Value::String(s) => s.value().clone(),
        Value::Integer(i) => i.value().to_string(),
        Value::Boolean(b) => b.value().to_string(),
        Value::Float(f) => f.value().to_string(),
        other => other.to_string().trim().to_string(),
    })
}

/// Walk/create the chain of section tables, returning the innermost one.
fn table_path<'a>(doc: &'a mut DocumentMut, sections: &[&str]) -> Result<&'a mut Table> {
    let mut table = doc.as_table_mut();
    for s in sections {
        let entry = table
            .entry(s)
            .or_insert_with(|| Item::Table(Table::new()));
        table = entry
            .as_table_mut()
            .ok_or_else(|| anyhow!("`{}` is not a table in the config", s))?;
    }
    Ok(table)
}

/// Parse `raw` into a typed [`Value`] according to `kind`.
fn coerce(kind: &SettingKind, raw: &str) -> Result<Value> {
    let raw = raw.trim();
    Ok(match kind {
        SettingKind::Bool => {
            let b = match raw.to_lowercase().as_str() {
                "true" | "yes" | "on" | "1" => true,
                "false" | "no" | "off" | "0" => false,
                _ => bail!("expected a boolean (true/false), got '{}'", raw),
            };
            Value::from(b)
        }
        SettingKind::Int => {
            let n: i64 = raw
                .parse()
                .with_context(|| format!("expected an integer, got '{}'", raw))?;
            Value::from(n)
        }
        SettingKind::Enum(choices) => {
            let matched = choices
                .iter()
                .find(|c| c.eq_ignore_ascii_case(raw))
                .ok_or_else(|| anyhow!("invalid value '{}'; choices: {}", raw, choices.join(", ")))?;
            Value::from((*matched).to_string())
        }
        SettingKind::Str => Value::from(raw.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(s: &str) -> DocumentMut {
        s.parse::<DocumentMut>().unwrap()
    }

    fn apply(d: &mut DocumentMut, key: &str, kind: &SettingKind, raw: &str) {
        let val = coerce(kind, raw).unwrap();
        let parts: Vec<&str> = key.split('.').collect();
        let (sections, last) = parts.split_at(parts.len() - 1);
        let table = table_path(d, sections).unwrap();
        let mut value = val;
        if let Some(existing) = table.get(last[0]).and_then(Item::as_value) {
            *value.decor_mut() = existing.decor().clone();
        }
        table.insert(last[0], Item::Value(value));
    }

    #[test]
    fn sets_int_and_preserves_comments() {
        let mut d = doc("[ui]\nlog_limit = 30  # default count\ncolors = true\n");
        apply(&mut d, "ui.log_limit", &SettingKind::Int, "100");
        let out = d.to_string();
        assert!(out.contains("log_limit = 100  # default count"), "{out}");
        assert!(out.contains("colors = true"));
    }

    #[test]
    fn sets_bool_and_string() {
        let mut d = doc("[commit]\nrequire_scope = false\n");
        apply(&mut d, "commit.require_scope", &SettingKind::Bool, "true");
        apply(&mut d, "general.default_branch", &SettingKind::Str, "trunk");
        let out = d.to_string();
        assert!(out.contains("require_scope = true"));
        assert!(out.contains("default_branch = \"trunk\""));
    }

    #[test]
    fn creates_missing_section() {
        let mut d = doc("[ui]\ncolors = true\n");
        apply(&mut d, "stage.confirm_revert", &SettingKind::Bool, "false");
        let out = d.to_string();
        assert!(out.contains("[stage]"));
        assert!(out.contains("confirm_revert = false"));
    }

    #[test]
    fn enum_validation_rejects_unknown() {
        assert!(coerce(&SettingKind::Enum(&["sharp", "rounded"]), "wavy").is_err());
        assert!(coerce(&SettingKind::Enum(&["sharp", "rounded"]), "ROUNDED").is_ok());
    }

    #[test]
    fn bool_and_int_validation() {
        assert!(coerce(&SettingKind::Bool, "maybe").is_err());
        assert!(coerce(&SettingKind::Int, "lots").is_err());
        assert!(coerce(&SettingKind::Int, "42").is_ok());
    }
}
