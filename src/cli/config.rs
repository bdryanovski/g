//! `g config` arguments.

use clap::Args;

/// Arguments for configuration-related commands.
#[derive(Args)]
pub struct ConfigArgs {
    /// Open config file in $EDITOR
    #[arg(long)]
    pub edit: bool,

    /// Print the path to the config file
    #[arg(long)]
    pub path: bool,

    /// List available themes (built-in + custom) and exit
    #[arg(long)]
    pub themes: bool,

    /// Get a config value
    pub key: Option<String>,
}
