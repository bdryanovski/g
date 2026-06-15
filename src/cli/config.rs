//! `g config` arguments.

use clap::{Args, Subcommand};

/// Arguments for `g config`.
///
/// `g config` is overloaded:
///
/// - Plain `g config`                      → show a human-readable summary.
/// - `g config <key>`                      → fuzzy-search the summary for `<key>`.
/// - `g config --get <key>`                → print the **exact** current value (scripting).
/// - `g config --list`                     → every editable scalar with its value + help.
/// - `g config --themes`                   → interactive theme picker (legacy entry point).
/// - `g config --edit`                     → open the file in `$EDITOR`.
/// - `g config --path`                     → print the path to the config file.
/// - `g config --menu`                     → interactive menu over the full schema.
/// - `g config set <key> <value>`          → validate against schema + persist.
#[derive(Args)]
pub struct ConfigArgs {
    /// Subcommand (currently only `set`).  When absent, the flags / `key`
    /// positional below take effect.
    #[command(subcommand)]
    pub cmd: Option<ConfigCmd>,

    /// Open config file in $EDITOR
    #[arg(long)]
    pub edit: bool,

    /// Print the path to the config file
    #[arg(long)]
    pub path: bool,

    /// List available themes (built-in + custom) and exit
    #[arg(long)]
    pub themes: bool,

    /// Print every editable scalar setting with its current value and help text.
    #[arg(long)]
    pub list: bool,

    /// Interactive menu: pick a setting, see its current value, choose a new one.
    #[arg(long)]
    pub menu: bool,

    /// Print the exact current value of `<key>` (scripting-friendly).
    /// Pair with a key positional: `g config --get ui.log_limit`.
    #[arg(long, value_name = "KEY")]
    pub get: Option<String>,

    /// Launch the interactive theme creator.  Writes a new TOML file under
    /// `~/.config/g/themes/<name>.toml` that extends an existing theme and
    /// overrides only the colors you choose.
    #[arg(long)]
    pub new_theme: bool,

    /// Optional positional key — when present alone, fuzzy-search the
    /// summary for matching lines (legacy behaviour).
    pub key: Option<String>,
}

/// Subcommands for `g config`.
#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Set a config key, validated against the editable schema.
    ///
    /// Comments and formatting in `config.toml` are preserved.
    Set {
        /// Dotted key path, e.g. `ui.log_limit` or `ui.theme`.
        key: String,
        /// New value.  Booleans accept `true`/`false`/`yes`/`no`/`on`/`off`.
        /// Enums must match one of the documented choices.
        value: String,
    },
}
