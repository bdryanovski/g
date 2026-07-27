//! `g workflow …` CLI argument definitions.
//!
//! Provides commands for managing customizable git workflows including:
//! - Lifecycle operations: start, finish, sync, publish
//! - Management: list, info, use, status
//! - Configuration: create, edit, init, validate
//! - Sharing: clone, export, import

use clap::{Args, Subcommand};

/// Workflow management commands for customizable git branching strategies.
///
/// Define and use custom workflows like Git Flow, GitHub Flow, trunk-based,
/// or create your own branching model.
#[derive(Subcommand)]
#[command(after_help = "Workflow overview:\n\
                  \n\
                  \x20 g workflow list              list all available workflows\n\
                  \x20 g workflow info gitflow      show workflow details with diagram\n\
                  \x20 g workflow use github-flow   switch to a workflow\n\
                  \n\
                  Branch lifecycle:\n\
                  \n\
                  \x20 g workflow start feature login   create a new branch\n\
                  \x20 g workflow sync                  update branch from source\n\
                  \x20 g workflow publish               push and create PR\n\
                  \x20 g workflow finish                merge to target branch(es)\n\
                  \n\
                  Configuration:\n\
                  \n\
                  \x20 g workflow create            interactive workflow builder\n\
                  \x20 g workflow init --local      set up .g/ folder in repo")]
pub enum WorkflowCommands {
    // ─── Lifecycle operations ─────────────────────────────────────────────────

    /// Start a new branch using workflow rules
    ///
    /// Creates a branch with the proper prefix, from the correct source branch,
    /// according to the workflow's branch type configuration.
    Start(StartArgs),

    /// Finish the current branch (merge to target)
    ///
    /// Merges the current branch to its configured target(s) using the
    /// appropriate merge strategy, then optionally deletes the branch
    /// and creates tags as configured.
    Finish(FinishArgs),

    /// Update branch from its source
    ///
    /// Fetches latest changes and rebases or merges from the source branch
    /// to keep the current branch up-to-date.
    Sync(SyncArgs),

    /// Push branch and create/update PR
    ///
    /// Pushes the branch to the remote and creates a pull request if one
    /// doesn't exist, or updates the existing PR.
    Publish(PublishArgs),

    // ─── Status and information ───────────────────────────────────────────────

    /// Show workflow status of current branch
    ///
    /// Displays the current branch's workflow context including type, source,
    /// target, merge strategy, age, and PR status.
    Status,

    /// List all available workflows
    ///
    /// Shows all defined workflows (built-in presets and custom) with their
    /// branch types and a brief description.
    List,

    /// Show detailed workflow information
    ///
    /// Displays the full workflow configuration including ASCII diagram,
    /// use cases, pros/cons, and branch type details.
    Info(InfoArgs),

    /// Switch to a different workflow
    ///
    /// Sets the active workflow for the current repository (if --local)
    /// or globally.
    Use(UseArgs),

    // ─── Configuration ────────────────────────────────────────────────────────

    /// Create a new workflow interactively
    ///
    /// Opens a full-screen wizard to define a custom workflow with branch
    /// types, merge strategies, hooks, and validation rules.
    Create(CreateArgs),

    /// Edit an existing workflow
    ///
    /// Opens the workflow configuration in your editor ($EDITOR) for
    /// direct modification.
    Edit(EditArgs),

    /// Initialize workflow configuration
    ///
    /// Sets up the workflow system for first use. With --local, creates
    /// a .g/ folder in the repository for team-shared configuration.
    Init(InitArgs),

    /// Validate workflow configuration
    ///
    /// Checks the workflow configuration for errors and warnings.
    Validate(ValidateArgs),

    // ─── Sharing ──────────────────────────────────────────────────────────────

    /// Clone a workflow with a new name
    ///
    /// Creates a copy of an existing workflow (preset or custom) that
    /// can be modified independently.
    Clone(CloneArgs),

    /// Export workflow configuration to TOML
    ///
    /// Prints the workflow configuration to stdout or writes to a file.
    Export(ExportArgs),

    /// Import workflow from a TOML file
    ///
    /// Loads a workflow configuration from a file and adds it to the
    /// available workflows.
    Import(ImportArgs),
}

impl WorkflowCommands {
    /// Return the subcommand name for telemetry.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Start(_) => "start",
            Self::Finish(_) => "finish",
            Self::Sync(_) => "sync",
            Self::Publish(_) => "publish",
            Self::Status => "status",
            Self::List => "list",
            Self::Info(_) => "info",
            Self::Use(_) => "use",
            Self::Create(_) => "create",
            Self::Edit(_) => "edit",
            Self::Init(_) => "init",
            Self::Validate(_) => "validate",
            Self::Clone(_) => "clone",
            Self::Export(_) => "export",
            Self::Import(_) => "import",
        }
    }
}

// ─── Lifecycle argument structs ───────────────────────────────────────────────

/// Arguments for `g workflow start`.
#[derive(Args)]
pub struct StartArgs {
    /// Branch type (e.g., feature, hotfix, release)
    #[arg(value_name = "TYPE")]
    pub branch_type: String,

    /// Branch name (without prefix)
    #[arg(value_name = "NAME")]
    pub name: String,

    /// Override the source branch
    #[arg(long, value_name = "BRANCH")]
    pub from: Option<String>,

    /// Skip validation checks
    #[arg(long)]
    pub no_verify: bool,
}

/// Arguments for `g workflow finish`.
#[derive(Args)]
pub struct FinishArgs {
    /// Branch to finish (defaults to current branch)
    #[arg(value_name = "BRANCH")]
    pub branch: Option<String>,

    /// Don't delete the branch after merge
    #[arg(long)]
    pub no_delete: bool,

    /// Don't create a tag even if configured
    #[arg(long)]
    pub no_tag: bool,

    /// Skip pre-finish hooks
    #[arg(long)]
    pub no_verify: bool,

    /// Override the merge strategy
    #[arg(long, value_name = "STRATEGY")]
    pub strategy: Option<String>,
}

/// Arguments for `g workflow sync`.
#[derive(Args)]
pub struct SyncArgs {
    /// Branch to sync (defaults to current branch)
    #[arg(value_name = "BRANCH")]
    pub branch: Option<String>,

    /// Force rebase even if merge is the default strategy
    #[arg(long)]
    pub rebase: bool,

    /// Force merge even if rebase is the default strategy
    #[arg(long, conflicts_with = "rebase")]
    pub merge: bool,
}

/// Arguments for `g workflow publish`.
#[derive(Args)]
pub struct PublishArgs {
    /// Branch to publish (defaults to current branch)
    #[arg(value_name = "BRANCH")]
    pub branch: Option<String>,

    /// Create PR as draft
    #[arg(long)]
    pub draft: bool,

    /// Skip on_publish hooks
    #[arg(long)]
    pub no_verify: bool,

    /// PR title (defaults to branch name)
    #[arg(long, value_name = "TITLE")]
    pub title: Option<String>,

    /// PR body
    #[arg(long, value_name = "BODY")]
    pub body: Option<String>,

    /// Add reviewers (comma-separated)
    #[arg(long, value_name = "USERS")]
    pub reviewers: Option<String>,

    /// Add labels (comma-separated)
    #[arg(long, value_name = "LABELS")]
    pub labels: Option<String>,
}

// ─── Information argument structs ─────────────────────────────────────────────

/// Arguments for `g workflow info`.
#[derive(Args)]
pub struct InfoArgs {
    /// Workflow name (preset or custom)
    #[arg(value_name = "NAME")]
    pub name: String,
}

/// Arguments for `g workflow use`.
#[derive(Args)]
pub struct UseArgs {
    /// Workflow name to activate
    #[arg(value_name = "NAME")]
    pub name: String,

    /// Set for this repository only (saves to .g/workflow.toml)
    #[arg(long)]
    pub local: bool,
}

// ─── Configuration argument structs ───────────────────────────────────────────

/// Arguments for `g workflow create`.
#[derive(Args)]
pub struct CreateArgs {
    /// Workflow name
    #[arg(value_name = "NAME")]
    pub name: Option<String>,

    /// Start from a preset
    #[arg(long, value_name = "PRESET")]
    pub from: Option<String>,

    /// Save to repo-local config (.g/workflow.toml)
    #[arg(long)]
    pub local: bool,

    /// Skip interactive wizard, save defaults immediately
    #[arg(long)]
    pub no_interactive: bool,
}

/// Arguments for `g workflow edit`.
#[derive(Args)]
pub struct EditArgs {
    /// Workflow name to edit
    #[arg(value_name = "NAME")]
    pub name: Option<String>,

    /// Edit raw TOML in $EDITOR
    #[arg(long)]
    pub raw: bool,
}

/// Arguments for `g workflow init`.
#[derive(Args)]
pub struct InitArgs {
    /// Create .g/ folder in repository for team-shared config
    #[arg(long)]
    pub local: bool,

    /// Use a preset as starting point
    #[arg(long, value_name = "PRESET")]
    pub preset: Option<String>,

    /// Skip interactive setup
    #[arg(long)]
    pub no_interactive: bool,
}

/// Arguments for `g workflow validate`.
#[derive(Args)]
pub struct ValidateArgs {
    /// File to validate (defaults to active config)
    #[arg(value_name = "FILE")]
    pub file: Option<String>,

    /// Workflow name to validate (within config)
    #[arg(long, value_name = "NAME")]
    pub workflow: Option<String>,
}

// ─── Sharing argument structs ─────────────────────────────────────────────────

/// Arguments for `g workflow clone`.
#[derive(Args)]
pub struct CloneArgs {
    /// Source workflow name
    #[arg(value_name = "SOURCE")]
    pub source: String,

    /// New workflow name
    #[arg(value_name = "NAME")]
    pub name: String,
}

/// Arguments for `g workflow export`.
#[derive(Args)]
pub struct ExportArgs {
    /// Workflow name to export
    #[arg(value_name = "NAME")]
    pub name: String,

    /// Output file (defaults to stdout)
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<String>,
}

/// Arguments for `g workflow import`.
#[derive(Args)]
pub struct ImportArgs {
    /// TOML file to import
    #[arg(value_name = "FILE")]
    pub file: String,

    /// Override workflow name
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,

    /// Save to repo-local config (.g/workflow.toml)
    #[arg(long)]
    pub local: bool,
}
