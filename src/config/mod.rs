//! Configuration types and config-file I/O.
//!
//! ## Tutorial overview
//!
//! This module defines the persistent configuration for the CLI, stored at
//! `~/.config/g/config.toml`.  It uses `serde` and `toml` to map Rust structs
//! directly to a human-readable file format, and provides helper functions for
//! locating, loading, saving, and bootstrapping the config.
//!
//! ## Rust concepts used here
//!
//! - `#[derive(Serialize, Deserialize)]` automatically generates TOML
//!   conversion code at compile time — no manual parsing needed.
//! - `HashMap<String, String>` for dynamic key-value storage (git aliases).
//! - The [`Default`] trait gives every config section a sensible baseline so
//!   users only need to override what they care about.
//! - `fs::create_dir_all` and `fs::write` for managing the local filesystem.
//! - `anyhow::Context` attaches a human-readable message to any `Result` error,
//!   making it much easier to diagnose file I/O failures.

use anyhow::{Context, Result};
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

// ─── Config structure ─────────────────────────────────────────────────────────

/// Root configuration struct — mirrors the top-level TOML table.
///
/// Every field is tagged with `#[serde(default)]` so that a minimal (or even
/// empty) config file is accepted; missing keys fall back to [`Default`].
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    /// General git settings.
    #[serde(default)]
    pub general: GeneralConfig,
    /// User-interface preferences.
    #[serde(default)]
    pub ui: UiConfig,
    /// Conventional Commit flow settings.
    #[serde(default)]
    pub commit: CommitConfig,
    /// Diff-tool selection and options.
    #[serde(default)]
    pub diff: DiffConfig,
    /// GitHub API integration.
    #[serde(default)]
    pub github: GithubConfig,
    /// Workspace (git worktree) settings.
    #[serde(default)]
    pub workspace: WorkspaceConfig,
    /// Log-output formatting settings.
    #[serde(default)]
    pub log: LogConfig,
    /// Interactive staging / unstaging settings.
    #[serde(default)]
    pub stage: StageConfig,
    /// User-defined command aliases (`co = "checkout"`, etc.).
    #[serde(default)]
    pub aliases: HashMap<String, String>,
    /// Plugin discovery configuration.
    #[serde(default)]
    pub plugins: PluginsConfig,
}

/// General settings: git executable path, default branch, auto-fetch, pager.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeneralConfig {
    /// Default branch name used as a base for comparisons and new stacks
    /// (e.g. `"main"`, `"master"`, `"trunk"`).
    pub default_branch: String,
    /// When `true`, `g compare` runs `git fetch --all` before computing
    /// ahead/behind counts so the numbers reflect the remote state.
    pub auto_fetch: bool,
    /// Optional pager program (`"delta"`, `"less"`, `"bat"`, or `""` to
    /// disable).  When `None`, the system default is used.
    pub pager: Option<String>,
    /// Override the path to the `git` executable.  Useful when multiple git
    /// versions are installed.  When `None`, `git` is resolved from `$PATH`.
    pub git_path: Option<String>,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            default_branch: "main".into(),
            auto_fetch: false,
            pager: None,
            git_path: None,
        }
    }
}

/// User-interface preferences: colours, icons, date format, log limits.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UiConfig {
    /// Enable ANSI-coloured output.
    pub colors: bool,
    /// Use Unicode icons and box-drawing characters.  Set to `false` for
    /// environments that only support ASCII.
    pub icons: bool,
    /// Date display format: `"relative"` (3 days ago), `"short"` (2024-01-15),
    /// `"iso"`, or `"rfc"`.
    pub date_format: String,
    /// Maximum number of commits shown by `g log` when `-n` is not given.
    pub log_limit: usize,
    /// Show the ASCII branch graph in `g log` output.
    pub show_graph: bool,
    /// Prompt rendering mode for the commit builder only.
    /// Superseded by [`prompt_mode`] when that is set.
    /// Accepted values: `"interactive"` (default) | `"inline"`.
    #[serde(default = "default_commit_mode")]
    pub commit_mode: String,
    /// Global prompt rendering mode — controls **all** interactive prompts
    /// (`g commit`, `g stage`, `g add`, `g workspace switch`, etc.).
    ///
    /// - `"interactive"` (default) — full-screen ratatui TUI, alternate screen.
    /// - `"inline"` — prompts render into the normal terminal scroll buffer;
    ///   no alternate screen is entered.  All output stays in history.
    ///
    /// When set to `"inline"` this also implies `commit_mode = "inline"`.
    #[serde(default = "default_prompt_mode")]
    pub prompt_mode: String,
    /// Color theme: `"dark"` (default) or `"light"`.
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Optional **override** of the border / box-drawing style. When unset
    /// (the default), the active theme decides — each theme file carries its
    /// own `border_style`. Set this to force one style regardless of theme:
    /// `"sharp"` | `"rounded"` | `"heavy"` | `"double"` | `"ascii"`.
    #[serde(default)]
    pub border_style: Option<String>,
    /// Optional **override** of layout density. When unset (the default), the
    /// active theme decides via its own `density`. Set to force one regardless
    /// of theme: `"normal"` | `"compact"` | `"relaxed"`.
    #[serde(default)]
    pub density: Option<String>,
}

fn default_commit_mode() -> String {
    "interactive".to_string()
}

fn default_prompt_mode() -> String {
    "interactive".to_string()
}

fn default_theme() -> String {
    "dark".to_string()
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            colors: true,
            icons: true,
            date_format: "relative".into(),
            log_limit: 30,
            show_graph: true,
            commit_mode: default_commit_mode(),
            prompt_mode: default_prompt_mode(),
            theme: default_theme(),
            border_style: None,
            density: None,
        }
    }
}

/// Settings for the interactive `g commit` flow.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct CommitConfig {
    /// Conventional Commit types shown in the interactive type picker.
    pub types: Vec<String>,
    /// When `true`, the scope prompt is mandatory.
    pub require_scope: bool,
    /// When `true`, the body prompt is mandatory.
    pub require_body: bool,
    /// When `true`, prompt the user to add a body during interactive commit.
    /// When `false`, the body prompt is skipped entirely (unless `require_body` is true).
    #[serde(default)]
    pub prompt_body: bool,
    /// When `true`, prompt the user to add a footer during interactive commit.
    /// When `false`, the footer prompt is skipped entirely.
    #[serde(default)]
    pub prompt_footer: bool,
    /// Optional custom commit-message template.
    pub template: Option<String>,
    /// Maximum subject-line length before a warning is shown (default: 72).
    pub max_subject_length: usize,
    /// Append a `Signed-off-by: Name <email>` trailer to every commit message
    /// (`-s` / `--signoff` flag to `git commit`).  The name and email are read
    /// from your git `user.name` / `user.email` config.
    #[serde(default)]
    pub sign_off: bool,
    /// Sign commits with GPG (`-S` / `--gpg-sign` flag to `git commit`).
    /// Requires a GPG key configured in git (`user.signingKey`).
    pub gpg_sign: bool,
    /// Show emoji next to commit type names in the interactive picker.
    pub emoji: bool,
}

impl Default for CommitConfig {
    fn default() -> Self {
        Self {
            types: vec![
                "feat".into(),
                "fix".into(),
                "docs".into(),
                "style".into(),
                "refactor".into(),
                "perf".into(),
                "test".into(),
                "build".into(),
                "ci".into(),
                "chore".into(),
                "revert".into(),
            ],
            require_scope: false,
            require_body: false,
            prompt_body: false,
            prompt_footer: false,
            template: None,
            max_subject_length: 72,
            sign_off: false,
            gpg_sign: false,
            emoji: false,
        }
    }
}

/// Diff-tool selection and context configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiffConfig {
    /// Diff tool to use: `"auto"` (detect `delta`/`diff-so-fancy` in `$PATH`),
    /// `"builtin"`, `"delta"`, `"diff-so-fancy"`, or a custom executable path.
    pub tool: String,
    /// Extra arguments forwarded to the diff tool.
    pub tool_args: Vec<String>,
    /// Number of context lines shown around each change hunk.
    pub context_lines: usize,
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            tool: "auto".into(),
            tool_args: vec![],
            context_lines: 3,
        }
    }
}

/// Interactive `g stage` settings.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StageConfig {
    /// When `true` (default), pressing `d` to revert a file shows a
    /// confirmation popup before discarding changes.  Set to `false`
    /// to revert immediately without asking.
    pub confirm_revert: bool,
}

impl Default for StageConfig {
    fn default() -> Self {
        Self {
            confirm_revert: true,
        }
    }
}

/// GitHub API integration settings.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GithubConfig {
    /// GitHub personal access token.  Prefer the `GITHUB_TOKEN` environment
    /// variable over storing a token in the config file.
    pub token: Option<String>,
    /// Default PR reviewers added to every newly created PR.
    pub default_reviewers: Vec<String>,
    /// Default labels applied to every newly created PR.
    pub default_labels: Vec<String>,
    /// GitHub API base URL.  Override for GitHub Enterprise instances
    /// (e.g. `"https://github.corp.example.com/api/v3"`).
    pub api_base: String,
}

impl Default for GithubConfig {
    fn default() -> Self {
        Self {
            token: None,
            default_reviewers: vec![],
            default_labels: vec![],
            api_base: "https://api.github.com".into(),
        }
    }
}

/// Workspace (git worktree) directory naming settings.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkspaceConfig {
    /// String placed between the repo name and the workspace name when
    /// constructing sibling worktree directories.
    ///
    /// For example, with `separator = "--"` and repo `myapp`, a workspace
    /// named `feature-x` is placed at `../myapp--feature-x`.
    pub separator: String,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            separator: "--".into(),
        }
    }
}

/// Log-formatting preferences.
///
/// All fields default to `None` / `false`, so this can use `#[derive(Default)]`
/// instead of a manual `impl` — less boilerplate and equivalent behaviour.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LogConfig {
    /// Custom `git log --format` string.  When `None`, the built-in pretty
    /// formatter is used.
    pub format: Option<String>,
    /// Show commit GPG signature status in log output.
    pub show_signature: bool,
    /// Show a diff-stat summary beneath each commit in log output.
    pub show_stat: bool,
}

/// Plugin discovery configuration.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PluginsConfig {
    /// Additional directories to scan for plugin executables.
    pub paths: Vec<String>,
    /// When `true`, executables named `g-<name>` anywhere in `$PATH` are
    /// treated as `g` subcommands.
    pub discover: bool,
}

// ─── Config I/O ──────────────────────────────────────────────────────────────

/// Return the `~/.config/g` configuration directory path.
///
/// # Errors
///
/// Returns an error if the user's home directory cannot be determined.
#[must_use = "use the returned path or it is wasted"]
pub fn config_dir() -> Result<PathBuf> {
    let home = home_dir().context("Could not find home directory")?;
    // We use APP_ID ("g") rather than the runtime binary name here
    // deliberately: the config directory must stay stable even if the binary
    // is renamed or symlinked.  Only changing APP_ID in main.rs should move
    // the config location.
    Ok(home.join(".config").join(crate::APP_ID))
}

/// Return the full path to the main `config.toml` file.
///
/// # Errors
///
/// Propagates any error from [`config_dir`].
#[must_use = "use the returned path or it is wasted"]
pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

/// Return the full path to the `workspaces.toml` metadata file.
///
/// # Errors
///
/// Propagates any error from [`config_dir`].
#[must_use = "use the returned path or it is wasted"]
pub fn workspaces_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("workspaces.toml"))
}

/// Return the full path to the `stacks.toml` metadata file.
///
/// # Errors
///
/// Propagates any error from [`config_dir`].
#[must_use = "use the returned path or it is wasted"]
pub fn stacks_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("stacks.toml"))
}

/// Return the full path to the `g.db` SQLite database file.
///
/// # Errors
///
/// Propagates any error from [`config_dir`].
#[must_use = "use the returned path or it is wasted"]
pub fn db_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("g.db"))
}

/// Ensure the config directory and a default `config.toml` exist on disk.
///
/// This is called once at startup.  If the directory or file are missing they
/// are created with sensible defaults and a message is printed to the user.
///
/// # Errors
///
/// Returns an error if:
/// - The config directory cannot be created.
/// - The default config file cannot be written.
pub fn ensure_config() -> Result<()> {
    let dir = config_dir()?;
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create config directory: {}", dir.display()))?;
    }

    let path = config_path()?;
    if !path.exists() {
        let default = default_config_toml();
        fs::write(&path, default)
            .with_context(|| format!("Failed to write default config: {}", path.display()))?;
        crate::ui::print_info(&format!("Created default config at {}", path.display()));
    }

    // Write editable copies of the built-in themes into ~/.config/g/themes so
    // users can tweak them without recompiling.  Existing files are preserved.
    if let Err(e) = crate::ui::theme::materialize_builtin_themes() {
        crate::ui::print_warning(&format!("Could not write built-in themes: {e}"));
    }

    Ok(())
}

/// Load and parse the config file from disk, merging with struct defaults.
///
/// If the config file does not exist yet, a [`Config::default()`] value is
/// returned so the caller never has to special-case a missing file.
///
/// # Errors
///
/// Returns an error if:
/// - The config path cannot be determined.
/// - The file exists but cannot be read (e.g. permission denied).
/// - The file content is not valid TOML or does not match the [`Config`] schema.
pub fn load() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config: {}", path.display()))?;
    let config: Config = toml::from_str(&raw)
        .with_context(|| format!("Failed to parse config: {}", path.display()))?;
    Ok(config)
}

/// Serialise `config` and write it to the config file on disk.
///
/// # Errors
///
/// Returns an error if:
/// - The config path cannot be determined.
/// - The struct cannot be serialised to TOML.
/// - The file cannot be written (e.g. permission denied, disk full).
#[allow(dead_code)]
pub fn save(config: &Config) -> Result<()> {
    let path = config_path()?;
    let raw = toml::to_string_pretty(config).context("Failed to serialize config")?;
    fs::write(&path, raw).with_context(|| format!("Failed to write config: {}", path.display()))?;
    Ok(())
}

/// Persist a new active theme name (`[ui] theme = "<name>"`).
///
/// This performs a *surgical* edit of the existing `config.toml` so the file's
/// comments and formatting are preserved: it rewrites only the `theme = …` line
/// (keeping its indentation and any trailing comment).  If the key is not found
/// — or no config file exists yet — it falls back to a structured load/save.
///
/// # Errors
///
/// Returns an error if the config path cannot be determined or the file cannot
/// be read or written.
pub fn set_theme(theme: &str) -> Result<()> {
    let path = config_path()?;
    if path.exists() {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config: {}", path.display()))?;
        if let Some(out) = replace_theme_line(&raw, theme) {
            fs::write(&path, out)
                .with_context(|| format!("Failed to write config: {}", path.display()))?;
            return Ok(());
        }
    }

    // Fallback: structured load/save (loses comments, but always correct).
    let mut cfg = load()?;
    cfg.ui.theme = theme.to_string();
    save(&cfg)
}

/// Rewrite the first `theme = …` line in `raw` to use `theme`, preserving
/// indentation and any trailing inline comment.  Returns `None` when no such
/// line exists (the caller then falls back to a structured save).
fn replace_theme_line(raw: &str, theme: &str) -> Option<String> {
    let mut replaced = false;
    let lines: Vec<String> = raw
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            let is_theme_key = trimmed
                .strip_prefix("theme")
                .map(|r| r.trim_start().starts_with('='))
                .unwrap_or(false);
            if !replaced && is_theme_key {
                replaced = true;
                let indent = &line[..line.len() - trimmed.len()];
                let comment = line
                    .find('#')
                    .map(|i| format!("  {}", &line[i..]))
                    .unwrap_or_default();
                format!("{indent}theme = \"{theme}\"{comment}")
            } else {
                line.to_string()
            }
        })
        .collect();

    if !replaced {
        return None;
    }
    let mut out = lines.join("\n");
    if raw.ends_with('\n') {
        out.push('\n');
    }
    Some(out)
}

// ─── Default config template ─────────────────────────────────────────────────

/// Returns the default `config.toml` content written on first run.
///
/// The template lives in `default_config.toml` next to this file and is
/// embedded into the binary via `include_str!` at compile time.  Editing it
/// only requires touching the `.toml` file — no Rust changes needed.
fn default_config_toml() -> &'static str {
    include_str!("default_config.toml")
}

impl Default for Config {
    fn default() -> Self {
        // Try to parse the built-in default template first so users get the
        // same values whether or not they have a config file.  Fall back to
        // constructing the struct manually if parsing somehow fails (e.g. the
        // template has a syntax error introduced during development).
        toml::from_str(default_config_toml()).unwrap_or_else(|_| Self {
            general: GeneralConfig::default(),
            ui: UiConfig::default(),
            commit: CommitConfig::default(),
            diff: DiffConfig::default(),
            github: GithubConfig::default(),
            workspace: WorkspaceConfig::default(),
            log: LogConfig::default(),
            stage: StageConfig::default(),
            aliases: HashMap::new(),
            plugins: PluginsConfig::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::replace_theme_line;

    #[test]
    fn preserves_trailing_comment_and_indent() {
        let raw = "[ui]\ntheme = \"dark\"   # \"dark\" | \"light\"\ncolors = true\n";
        let out = replace_theme_line(raw, "nord").unwrap();
        assert!(out.contains("theme = \"nord\"  # \"dark\" | \"light\""));
        assert!(out.contains("colors = true"));
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn replaces_only_first_theme_line() {
        let raw = "theme = \"dark\"\ntheme = \"light\"\n";
        let out = replace_theme_line(raw, "nord").unwrap();
        assert_eq!(out, "theme = \"nord\"\ntheme = \"light\"\n");
    }

    #[test]
    fn returns_none_without_theme_key() {
        assert!(replace_theme_line("colors = true\n", "nord").is_none());
    }

    #[test]
    fn ignores_unrelated_keys_with_theme_prefix() {
        // A key like `theme_extra` must not be mistaken for `theme`.
        let raw = "theme_extra = \"x\"\ntheme = \"dark\"\n";
        let out = replace_theme_line(raw, "nord").unwrap();
        assert_eq!(out, "theme_extra = \"x\"\ntheme = \"nord\"\n");
    }
}
