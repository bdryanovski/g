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
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            colors: true,
            icons: true,
            date_format: "relative".into(),
            log_limit: 30,
            show_graph: true,
        }
    }
}

/// Settings for the interactive `g commit` flow.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommitConfig {
    /// Conventional Commit types shown in the interactive type picker.
    pub types: Vec<String>,
    /// When `true`, the scope prompt is mandatory.
    pub require_scope: bool,
    /// When `true`, the body prompt is mandatory.
    pub require_body: bool,
    /// Optional custom commit-message template.
    pub template: Option<String>,
    /// Maximum subject-line length before a warning is shown (default: 72).
    pub max_subject_length: usize,
    /// Sign commits with GPG (`-S` flag to `git commit`).
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
            template: None,
            max_subject_length: 72,
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

// ─── Default config template ─────────────────────────────────────────────────

/// Returns the default `config.toml` content written on first run.
///
/// Using a raw string literal (`r#"…"#`) lets us embed a multi-line TOML
/// document without any escaping.
fn default_config_toml() -> &'static str {
    r#"# g configuration
# Documentation: https://github.com/your-org/g/

# ─── General ──────────────────────────────────────────────────────────────────
[general]
# Default branch name for new repositories and comparisons
default_branch = "main"
# Automatically run `git fetch` before branch comparisons
auto_fetch = false
# Override pager: "delta" | "less" | "bat" | "" (to disable)
# pager = "less"
# Override git executable path
# git_path = "/usr/bin/git"

# ─── User Interface ───────────────────────────────────────────────────────────
[ui]
colors = true
icons = true                # Unicode icons and box-drawing characters
date_format = "relative"    # "relative" | "short" | "iso" | "rfc"
log_limit = 30              # Default number of commits in log
show_graph = true           # Show branch graph in log

# ─── Commit Templates ─────────────────────────────────────────────────────────
[commit]
# Conventional commit types shown in interactive mode
types = [
    "feat",     # A new feature
    "fix",      # A bug fix
    "docs",     # Documentation only changes
    "style",    # Formatting, missing semi colons, etc
    "refactor", # Code change that neither fixes a bug nor adds a feature
    "perf",     # A code change that improves performance
    "test",     # Adding missing tests
    "build",    # Changes to build system or dependencies
    "ci",       # Changes to CI configuration
    "chore",    # Other changes that don't modify src or test files
    "revert",   # Reverts a previous commit
]
require_scope = false   # Require a scope in commit messages
require_body = false    # Require a body in commit messages
max_subject_length = 72 # Maximum subject line length
gpg_sign = false        # Sign commits with GPG

# Custom commit template (optional)
# template = """
# {type}({scope}): {subject}
#
# {body}
#
# {footer}
# """

# ─── Diff Tool ────────────────────────────────────────────────────────────────
[diff]
# "auto" detects delta/diff-so-fancy in PATH, falls back to builtin
# Other options: "delta" | "diff-so-fancy" | "vimdiff" | "/path/to/tool"
tool = "auto"
tool_args = []
context_lines = 3

# ─── GitHub Integration ───────────────────────────────────────────────────────
[github]
# GitHub token — prefer setting GITHUB_TOKEN environment variable
# token = ""
default_reviewers = []
default_labels = []
# For GitHub Enterprise:
# api_base = "https://github.your-company.com/api/v3"
api_base = "https://api.github.com"

# ─── Workspace ────────────────────────────────────────────────────────────────
[workspace]
# Separator between repo name and workspace name for sibling worktree directories
# e.g. with "--": ~/proj/myapp--feature-x
separator = "--"

# ─── Aliases ──────────────────────────────────────────────────────────────────
[aliases]
co = "checkout"
br = "branch"
st = "status"
lg = "log"
cp = "cherry-pick"
rb = "rebase"
sw = "switch"

# ─── Plugins ──────────────────────────────────────────────────────────────────
[plugins]
# Discover commands named "g-<name>" in PATH
discover = true
# Additional plugin paths
paths = []
"#
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
            aliases: HashMap::new(),
            plugins: PluginsConfig::default(),
        })
    }
}
