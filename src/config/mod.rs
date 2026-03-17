//! Configuration types and config file I/O.
//!
//! This module defines the user-facing configuration schema and helpers
//! to load/save TOML files under `~/.config/g`.

use anyhow::{Context, Result};
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

// ─── Config Structure ────────────────────────────────────────────────────────

/// Root configuration struct (matches the TOML schema).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub commit: CommitConfig,
    #[serde(default)]
    pub diff: DiffConfig,
    #[serde(default)]
    pub github: GithubConfig,
    #[serde(default)]
    pub workspace: WorkspaceConfig,
    #[serde(default)]
    pub log: LogConfig,
    #[serde(default)]
    pub aliases: HashMap<String, String>,
    #[serde(default)]
    pub plugins: PluginsConfig,
}

/// General settings (git path, default branch, etc.).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeneralConfig {
    /// Default branch name (main, master, trunk, etc.)
    pub default_branch: String,
    /// Automatically fetch before comparing
    pub auto_fetch: bool,
    /// Pager program to use
    pub pager: Option<String>,
    /// Path to git executable (defaults to searching PATH)
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

/// UI-related settings.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UiConfig {
    /// Enable colored output
    pub colors: bool,
    /// Use Unicode icons and box-drawing characters
    pub icons: bool,
    /// Date format: "relative" | "short" | "iso" | "rfc"
    pub date_format: String,
    /// How many commits to show in log by default
    pub log_limit: usize,
    /// Show commit graph in log
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

/// Commit-flow settings.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommitConfig {
    /// Conventional commit types
    pub types: Vec<String>,
    /// Whether scope is required
    pub require_scope: bool,
    /// Whether body is required
    pub require_body: bool,
    /// Custom template for commit messages
    pub template: Option<String>,
    /// Max subject line length
    pub max_subject_length: usize,
    /// Sign commits with GPG
    pub gpg_sign: bool,
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
        }
    }
}

/// Diff-tool settings.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiffConfig {
    /// Diff tool: "builtin" | "delta" | "diff-so-fancy" | custom path
    pub tool: String,
    /// Extra args to pass to the diff tool
    pub tool_args: Vec<String>,
    /// Context lines to show
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

/// GitHub integration settings.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GithubConfig {
    /// GitHub personal access token (prefer GITHUB_TOKEN env var)
    pub token: Option<String>,
    /// Default PR reviewers
    pub default_reviewers: Vec<String>,
    /// Default PR labels
    pub default_labels: Vec<String>,
    /// GitHub API base URL (for GitHub Enterprise)
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

/// Workspace management settings.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkspaceConfig {
    /// Separator between repo name and workspace name in sibling directories
    pub separator: String,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            separator: "--".into(),
        }
    }
}

/// Log-formatting settings.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogConfig {
    /// Format string for log output (git format)
    pub format: Option<String>,
    /// Show commit signature status
    pub show_signature: bool,
    /// Show diff stat in log
    pub show_stat: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            format: None,
            show_signature: false,
            show_stat: false,
        }
    }
}

/// Plugin discovery settings.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PluginsConfig {
    /// Paths to plugin scripts/binaries
    pub paths: Vec<String>,
    /// Whether to load plugins from $PATH with prefix "g"
    pub discover: bool,
}

// ─── Config I/O ──────────────────────────────────────────────────────────────

/// Return the `~/.config/g` directory.
pub fn config_dir() -> Result<PathBuf> {
    let home = home_dir().context("Could not find home directory")?;
    Ok(home.join(".config").join("g"))
}

/// Return the full path to `config.toml`.
pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

/// Return the full path to `workspaces.toml`.
pub fn workspaces_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("workspaces.toml"))
}

/// Return the full path to `stacks.toml`.
pub fn stacks_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("stacks.toml"))
}

/// Ensure the config directory and default config file exist.
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
        crate::ui::print_info(&format!(
            "Created default config at {}",
            path.display()
        ));
    }

    Ok(())
}

/// Load config from disk, merging with defaults.
pub fn load() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config: {}", path.display()))?;
    let config: Config =
        toml::from_str(&raw).with_context(|| format!("Failed to parse config: {}", path.display()))?;
    Ok(config)
}

/// Save config to disk.
#[allow(dead_code)]
pub fn save(config: &Config) -> Result<()> {
    let path = config_path()?;
    let raw = toml::to_string_pretty(config).context("Failed to serialize config")?;
    fs::write(&path, raw)
        .with_context(|| format!("Failed to write config: {}", path.display()))?;
    Ok(())
}

// ─── Default Config Template ─────────────────────────────────────────────────

/// Default config template written when no config exists.
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

// TODO(config): Support layered config (system + user + repo-local overrides).
// TODO(config): Validate config values and surface friendly errors (e.g., bad date_format).
