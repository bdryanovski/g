//! Git hooks configuration types.
//!
//! Personal git hooks that run alongside (not replacing) existing hook systems
//! like Husky. Hooks can be configured per-repo locally or globally.
//!
//! ## Configuration locations (in priority order)
//!
//! 1. `.g/hooks.toml` — repo-local, gitignored (highest priority)
//! 2. `~/.config/g/hooks/<repo-name>.toml` — per-repo global config
//! 3. `~/.config/g/hooks/default.toml` — fallback for all repos

use serde::{Deserialize, Serialize};

/// Root hooks configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HooksConfig {
    /// Master switch to enable/disable all hooks.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Pre-commit hooks — run before `git commit`.
    #[serde(default, rename = "pre-commit")]
    pub pre_commit: Option<HookConfig>,

    /// Post-commit hooks — run after `git commit`.
    #[serde(default, rename = "post-commit")]
    pub post_commit: Option<HookConfig>,

    /// Pre-push hooks — run before `git push`.
    #[serde(default, rename = "pre-push")]
    pub pre_push: Option<HookConfig>,

    /// Post-checkout hooks — run after `git checkout`.
    #[serde(default, rename = "post-checkout")]
    pub post_checkout: Option<HookConfig>,

    /// Post-merge hooks — run after `git merge`.
    #[serde(default, rename = "post-merge")]
    pub post_merge: Option<HookConfig>,

    /// Pre-rebase hooks — run before `git rebase`.
    #[serde(default, rename = "pre-rebase")]
    pub pre_rebase: Option<HookConfig>,
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            pre_commit: None,
            post_commit: None,
            pre_push: None,
            post_checkout: None,
            post_merge: None,
            pre_rebase: None,
        }
    }
}

fn default_enabled() -> bool {
    true
}

/// Configuration for a single hook type.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HookConfig {
    /// Enable/disable this specific hook.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Only run if staged files match these glob patterns.
    /// Empty means run regardless of staged files.
    #[serde(default)]
    pub staged_patterns: Vec<String>,

    /// Pass matching staged files as arguments to the command.
    #[serde(default)]
    pub pass_files: bool,

    /// Commands to run (in order). First failure aborts.
    #[serde(default)]
    pub commands: Vec<HookCommand>,
}

impl Default for HookConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            staged_patterns: Vec::new(),
            pass_files: false,
            commands: Vec::new(),
        }
    }
}

/// A single command to run as part of a hook.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HookCommand {
    /// The shell command to execute.
    pub run: String,

    /// Display name for the command (shown in output).
    #[serde(default)]
    pub name: Option<String>,

    /// Working directory (relative to repo root). Defaults to repo root.
    #[serde(default)]
    pub cwd: Option<String>,

    /// Only run if these glob patterns match staged files.
    /// Overrides the hook-level `staged_patterns` if set.
    #[serde(default)]
    pub staged_patterns: Option<Vec<String>>,

    /// Pass matching staged files as arguments.
    /// Overrides the hook-level `pass_files` if set.
    #[serde(default)]
    pub pass_files: Option<bool>,
}

impl HookCommand {
    /// Get the display name for this command.
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.run)
    }
}

/// Hook types supported by the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookType {
    PreCommit,
    PostCommit,
    PrePush,
    PostCheckout,
    PostMerge,
    PreRebase,
}

impl HookType {
    /// Get the hook name as a string (for display and config keys).
    pub fn as_str(&self) -> &'static str {
        match self {
            HookType::PreCommit => "pre-commit",
            HookType::PostCommit => "post-commit",
            HookType::PrePush => "pre-push",
            HookType::PostCheckout => "post-checkout",
            HookType::PostMerge => "post-merge",
            HookType::PreRebase => "pre-rebase",
        }
    }

    /// Whether this hook can abort the operation (pre-* hooks).
    pub fn is_blocking(&self) -> bool {
        matches!(
            self,
            HookType::PreCommit | HookType::PrePush | HookType::PreRebase
        )
    }
}

impl std::fmt::Display for HookType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl HooksConfig {
    /// Get the configuration for a specific hook type.
    pub fn get(&self, hook_type: HookType) -> Option<&HookConfig> {
        match hook_type {
            HookType::PreCommit => self.pre_commit.as_ref(),
            HookType::PostCommit => self.post_commit.as_ref(),
            HookType::PrePush => self.pre_push.as_ref(),
            HookType::PostCheckout => self.post_checkout.as_ref(),
            HookType::PostMerge => self.post_merge.as_ref(),
            HookType::PreRebase => self.pre_rebase.as_ref(),
        }
    }

    /// Check if any hooks are configured.
    pub fn has_hooks(&self) -> bool {
        self.pre_commit.is_some()
            || self.post_commit.is_some()
            || self.pre_push.is_some()
            || self.post_checkout.is_some()
            || self.post_merge.is_some()
            || self.pre_rebase.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hooks_config() {
        let toml = r#"
enabled = true

[pre-commit]
enabled = true
staged_patterns = ["*.rs", "*.ts"]
pass_files = true
commands = [
    { run = "cargo fmt --check", name = "rustfmt" },
    { run = "cargo clippy", name = "clippy" },
]

[pre-push]
commands = [
    { run = "cargo test", name = "tests" },
]
"#;

        let config: HooksConfig = toml::from_str(toml).unwrap();
        assert!(config.enabled);

        let pre_commit = config.pre_commit.unwrap();
        assert!(pre_commit.enabled);
        assert_eq!(pre_commit.staged_patterns, vec!["*.rs", "*.ts"]);
        assert!(pre_commit.pass_files);
        assert_eq!(pre_commit.commands.len(), 2);
        assert_eq!(pre_commit.commands[0].name, Some("rustfmt".to_string()));

        let pre_push = config.pre_push.unwrap();
        assert_eq!(pre_push.commands.len(), 1);
    }

    #[test]
    fn hook_type_blocking() {
        assert!(HookType::PreCommit.is_blocking());
        assert!(HookType::PrePush.is_blocking());
        assert!(!HookType::PostCommit.is_blocking());
        assert!(!HookType::PostCheckout.is_blocking());
    }
}
