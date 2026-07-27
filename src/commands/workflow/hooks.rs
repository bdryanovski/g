//! Workflow hook execution engine.
//!
//! Executes shell commands at workflow lifecycle events with proper
//! environment variables and error handling.

use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::process::Command;

use crate::config::workflow::{BranchType, Workflow, WorkflowHooks};
use crate::ui;

/// Environment variables passed to hooks.
pub struct HookEnv {
    /// Active workflow name
    pub workflow: String,
    /// Branch type name (feature, hotfix, etc.)
    pub branch_type: String,
    /// Full branch name
    pub branch_name: String,
    /// Source branch
    pub source: String,
    /// Target branch(es), comma-separated
    pub target: String,
    /// Ticket ID (if extracted from branch name)
    pub ticket: Option<String>,
}

impl HookEnv {
    /// Create a new hook environment.
    pub fn new(
        workflow_name: &str,
        branch_type: &BranchType,
        branch_name: &str,
        source: &str,
        target: &str,
    ) -> Self {
        Self {
            workflow: workflow_name.to_string(),
            branch_type: branch_type.name.clone(),
            branch_name: branch_name.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            ticket: None,
        }
    }

    /// Set the ticket ID (extracted from branch name).
    pub fn with_ticket(mut self, ticket: Option<String>) -> Self {
        self.ticket = ticket;
        self
    }

    /// Convert to environment variable map.
    fn to_env_map(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();
        env.insert("G_WORKFLOW".to_string(), self.workflow.clone());
        env.insert("G_BRANCH_TYPE".to_string(), self.branch_type.clone());
        env.insert("G_BRANCH_NAME".to_string(), self.branch_name.clone());
        env.insert("G_SOURCE".to_string(), self.source.clone());
        env.insert("G_TARGET".to_string(), self.target.clone());
        if let Some(ref ticket) = self.ticket {
            env.insert("G_TICKET".to_string(), ticket.clone());
        }
        env
    }
}

/// Hook execution result.
pub enum HookResult {
    /// All hooks passed
    Success,
    /// No hooks configured
    NoHooks,
    /// Hook failed with message
    Failed(String),
}

/// Run a list of hook commands.
///
/// Returns error if any command fails (non-zero exit).
pub fn run_hooks(
    hooks: &[String],
    env: &HookEnv,
    dry_run: bool,
) -> Result<HookResult> {
    if hooks.is_empty() {
        return Ok(HookResult::NoHooks);
    }

    let env_map = env.to_env_map();

    for cmd in hooks {
        if dry_run {
            ui::print_info(&format!("Would run hook: {}", cmd));
            continue;
        }

        ui::print_info(&format!("Running hook: {}", cmd));

        let status = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .envs(&env_map)
            .status()
            .with_context(|| format!("Failed to execute hook: {}", cmd))?;

        if !status.success() {
            let code = status.code().unwrap_or(-1);
            return Ok(HookResult::Failed(format!(
                "Hook '{}' failed with exit code {}",
                cmd, code
            )));
        }
    }

    Ok(HookResult::Success)
}

/// Run pre_start hooks.
pub fn run_pre_start(
    hooks: &Option<WorkflowHooks>,
    env: &HookEnv,
    dry_run: bool,
) -> Result<()> {
    if let Some(ref h) = hooks {
        if let Some(ref cmds) = h.pre_start {
            match run_hooks(cmds, env, dry_run)? {
                HookResult::Failed(msg) => bail!("{}", msg),
                _ => {}
            }
        }
    }
    Ok(())
}

/// Run post_start hooks.
pub fn run_post_start(
    hooks: &Option<WorkflowHooks>,
    env: &HookEnv,
    dry_run: bool,
) -> Result<()> {
    if let Some(ref h) = hooks {
        if let Some(ref cmds) = h.post_start {
            match run_hooks(cmds, env, dry_run)? {
                HookResult::Failed(msg) => {
                    ui::print_warning(&format!("Post-start hook failed: {}", msg));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// Run pre_finish hooks.
///
/// Returns error if any hook fails.
pub fn run_pre_finish(
    hooks: &Option<WorkflowHooks>,
    env: &HookEnv,
    dry_run: bool,
) -> Result<()> {
    if let Some(ref h) = hooks {
        if let Some(ref cmds) = h.pre_finish {
            match run_hooks(cmds, env, dry_run)? {
                HookResult::Failed(msg) => bail!("{}", msg),
                _ => {}
            }
        }
    }
    Ok(())
}

/// Run post_finish hooks.
pub fn run_post_finish(
    hooks: &Option<WorkflowHooks>,
    env: &HookEnv,
    dry_run: bool,
) -> Result<()> {
    if let Some(ref h) = hooks {
        if let Some(ref cmds) = h.post_finish {
            match run_hooks(cmds, env, dry_run)? {
                HookResult::Failed(msg) => {
                    ui::print_warning(&format!("Post-finish hook failed: {}", msg));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// Run on_publish hooks.
pub fn run_on_publish(
    hooks: &Option<WorkflowHooks>,
    env: &HookEnv,
    dry_run: bool,
) -> Result<()> {
    if let Some(ref h) = hooks {
        if let Some(ref cmds) = h.on_publish {
            match run_hooks(cmds, env, dry_run)? {
                HookResult::Failed(msg) => bail!("{}", msg),
                _ => {}
            }
        }
    }
    Ok(())
}

/// Extract ticket ID from branch name using workflow's ticket pattern.
pub fn extract_ticket(workflow: &Workflow, branch_name: &str) -> Option<String> {
    let pattern = workflow.ticket_pattern.as_ref()?;
    let re = regex::Regex::new(pattern).ok()?;
    re.find(branch_name).map(|m| m.as_str().to_string())
}
