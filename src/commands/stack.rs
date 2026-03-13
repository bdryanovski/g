use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;

use crate::commands::git as gitcmd;
use crate::config;
use crate::github;
use crate::ui;

// ─── Data Structures ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Stack {
    /// User-facing name for the stack
    pub name: String,
    /// The base branch this stack is built on (e.g., "main")
    pub root: String,
    /// Ordered branches from bottom (closest to root) to top
    pub branches: Vec<StackBranch>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StackBranch {
    pub name: String,
    pub pr_number: Option<u64>,
    pub pr_url: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct StackStore {
    pub stacks: Vec<Stack>,
    /// Maps a branch name to which stack it belongs
    pub branch_to_stack: std::collections::HashMap<String, String>,
}

// ─── Persistence ─────────────────────────────────────────────────────────────

fn load_store() -> Result<StackStore> {
    let path = config::stacks_path()?;
    if !path.exists() {
        return Ok(StackStore::default());
    }
    let raw = fs::read_to_string(&path).context("Failed to read stacks file")?;
    toml::from_str(&raw).context("Failed to parse stacks file")
}

fn save_store(store: &StackStore) -> Result<()> {
    let path = config::stacks_path()?;
    let raw = toml::to_string_pretty(store).context("Failed to serialize stacks")?;
    fs::write(&path, raw).context("Failed to save stacks file")
}

fn current_stack(store: &StackStore) -> Result<&Stack> {
    let branch = gitcmd::current_branch()?;
    let stack_name = store
        .branch_to_stack
        .get(&branch)
        .with_context(|| format!("Branch '{}' is not part of any stack. Use `vcli stack new <name>` to create one.", branch))?;
    store
        .stacks
        .iter()
        .find(|s| &s.name == stack_name)
        .with_context(|| format!("Stack '{}' not found in store.", stack_name))
}

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Initialize a new stack rooted at the current branch
pub fn new_stack(name: &str) -> Result<()> {
    let mut store = load_store()?;
    let branch = gitcmd::current_branch()?;

    if store.stacks.iter().any(|s| s.name == name) {
        bail!("Stack '{}' already exists.", name);
    }

    let now = Utc::now();
    let stack = Stack {
        name: name.to_string(),
        root: branch.clone(),
        branches: vec![StackBranch {
            name: branch.clone(),
            pr_number: None,
            pr_url: None,
            description: None,
        }],
        created_at: now,
        updated_at: now,
    };

    store.branch_to_stack.insert(branch.clone(), name.to_string());
    store.stacks.push(stack);
    save_store(&store)?;

    println!();
    ui::print_success(&format!(
        "Created stack {} rooted at {}",
        name.cyan().bold(),
        branch.green().bold()
    ));
    println!();
    Ok(())
}

/// Add a new branch on top of the current stack position
pub fn add_branch(branch_name: &str) -> Result<()> {
    let mut store = load_store()?;
    let current_branch = gitcmd::current_branch()?;

    // Read-only pass: get stack name and position
    let (stack_name, current_pos) = {
        let stack = current_stack(&store)?;
        let pos = stack
            .branches
            .iter()
            .position(|b| b.name == current_branch)
            .with_context(|| format!("Branch '{}' not in stack", current_branch))?;
        (stack.name.clone(), pos)
    };

    // Create the git branch
    gitcmd::git_output(&["checkout", "-b", branch_name])
        .with_context(|| format!("Failed to create branch '{}'", branch_name))?;

    // Mutable pass: update the store
    let stack = store
        .stacks
        .iter_mut()
        .find(|s| s.name == stack_name)
        .with_context(|| format!("Stack '{}' disappeared", stack_name))?;

    stack.branches.insert(
        current_pos + 1,
        StackBranch {
            name: branch_name.to_string(),
            pr_number: None,
            pr_url: None,
            description: None,
        },
    );
    stack.updated_at = Utc::now();
    store.branch_to_stack.insert(branch_name.to_string(), stack_name);
    save_store(&store)?;

    println!();
    ui::print_success(&format!(
        "Created branch {} and added to stack",
        branch_name.green().bold()
    ));
    println!();
    Ok(())
}

pub fn list() -> Result<()> {
    let store = load_store()?;
    let current_branch = gitcmd::current_branch().unwrap_or_default();

    if store.stacks.is_empty() {
        println!();
        println!("  {}", "No stacks yet.".bright_black());
        println!(
            "  {} {}",
            "tip:".bright_black(),
            "vcli stack new <name>  to create a stack from the current branch".bright_black()
        );
        println!();
        return Ok(());
    }

    for stack in &store.stacks {
        println!();
        println!(
            "  {} {}  {}",
            "Stack:".bright_black(),
            stack.name.cyan().bold(),
            format!("(root: {})", stack.root).bright_black()
        );
        println!();

        let last = stack.branches.len().saturating_sub(1);
        for (i, branch) in stack.branches.iter().enumerate() {
            let is_current = branch.name == current_branch;
            let connector = if i == last { "  └──" } else { "  ├──" };
            let marker = if is_current {
                "◉".green().bold().to_string()
            } else {
                "◯".bright_black().to_string()
            };
            let name_colored = if is_current {
                branch.name.green().bold().to_string()
            } else {
                branch.name.white().to_string()
            };

            print!(
                "{} {} {}",
                connector.bright_black(),
                marker,
                name_colored
            );

            if let Some(pr_url) = &branch.pr_url {
                let pr_num = branch.pr_number.map(|n| format!(" #{}", n)).unwrap_or_default();
                print!("  {}{}", "PR".bright_black(), pr_num.cyan());
                print!("  {}", pr_url.bright_black().underline());
            }

            if is_current {
                print!("  {}", "← you are here".bright_black());
            }
            println!();

            if i < last {
                println!("  {}   {}", "│".bright_black(), "│".bright_black());
            }
        }
    }
    println!();
    Ok(())
}

pub fn view() -> Result<()> {
    list()
}

/// Sync all branches in the stack (rebase each on the one below)
pub fn sync(no_interactive: bool) -> Result<()> {
    let store = load_store()?;
    let stack = current_stack(&store)?.clone();

    println!();
    println!(
        "  {} {}",
        "Syncing stack:".bold().white(),
        stack.name.cyan().bold()
    );
    println!();

    let saved_branch = gitcmd::current_branch()?;

    for i in 1..stack.branches.len() {
        let base = stack.branches[i - 1].name.clone();
        let branch = stack.branches[i].name.clone();

        let pb = ui::spinner(&format!(
            "Rebasing {} onto {}",
            branch.green(),
            base.cyan()
        ));

        gitcmd::git_output(&["checkout", &branch])
            .with_context(|| format!("Failed to checkout '{}'", branch))?;

        let result = gitcmd::git_output(&["rebase", &base]);

        pb.finish_and_clear();

        match result {
            Ok(_) => {
                ui::print_success(&format!(
                    "{} rebased onto {}",
                    branch.green().bold(),
                    base.cyan()
                ));
            }
            Err(e) => {
                if no_interactive {
                    // Abort and bail
                    let _ = gitcmd::git_output(&["rebase", "--abort"]);
                    bail!(
                        "Conflict rebasing '{}' onto '{}': {}\nRun without --no-interactive to resolve manually.",
                        branch, base, e
                    );
                } else {
                    ui::print_warning(&format!(
                        "Conflict in {}: resolve manually, then run `vcli stack sync` again",
                        branch.yellow()
                    ));
                    println!();
                    println!("  {} After resolving conflicts:", "→".cyan());
                    println!("    {} git add <files>", "1.".bright_black());
                    println!("    {} git rebase --continue", "2.".bright_black());
                    println!("    {} vcli stack sync  (to continue remaining branches)", "3.".bright_black());
                    println!();
                    return Ok(());
                }
            }
        }
    }

    // Return to original branch
    let _ = gitcmd::git_output(&["checkout", &saved_branch]);

    println!();
    ui::print_success("Stack sync complete!");
    println!();
    Ok(())
}

/// Force-push all branches in the stack to their remotes
pub fn push(force: bool) -> Result<()> {
    let store = load_store()?;
    let stack = current_stack(&store)?.clone();

    println!();
    println!(
        "  {} {}",
        "Pushing stack:".bold().white(),
        stack.name.cyan().bold()
    );
    println!();

    for branch_entry in &stack.branches {
        let branch = &branch_entry.name;
        let pb = ui::spinner(&format!("Pushing {}", branch.green()));

        let push_args: Vec<&str> = if force {
            vec!["push", "origin", branch, "--force-with-lease"]
        } else {
            vec!["push", "origin", branch]
        };

        let result = gitcmd::git_output(&push_args);
        pb.finish_and_clear();

        match result {
            Ok(_) => ui::print_success(&format!("Pushed {}", branch.green().bold())),
            Err(e) => {
                ui::print_error(&format!("Failed to push {}: {}", branch.red(), e));
                if !force {
                    println!(
                        "  {} try {}",
                        "tip:".bright_black(),
                        "vcli stack push --force".yellow()
                    );
                }
            }
        }
    }

    println!();
    Ok(())
}

/// Create or update GitHub PRs for all branches in the stack
pub fn create_prs(open: bool, draft: bool) -> Result<()> {
    let mut store = load_store()?;
    let stack = current_stack(&store)?.clone();
    let cfg = config::load()?;

    let token = get_github_token(&cfg)?;
    let (owner, repo) = github::detect_repo()?;

    println!();
    println!(
        "  {} {} → {}/{}",
        "Creating PRs for stack:".bold().white(),
        stack.name.cyan().bold(),
        owner.bright_white(),
        repo.bright_white()
    );
    println!();

    // Skip the root branch (index 0), create PRs for branches 1..n
    for i in 1..stack.branches.len() {
        let base = stack.branches[i - 1].name.clone();
        let branch = stack.branches[i].name.clone();

        let pb = ui::spinner(&format!("Creating PR: {} → {}", branch.green(), base.cyan()));

        // Check if PR already exists for this branch
        let existing = github::find_pr(&token, &cfg.github.api_base, &owner, &repo, &branch)?;

        let result: Result<crate::github::PrInfo, anyhow::Error> = if let Some(pr) = existing {
            if pr.base_ref != base {
                pb.set_message(format!(
                    "Updating PR #{} base: {} → {}",
                    pr.number,
                    pr.base_ref.red(),
                    base.green()
                ));
                let updated = github::update_pr_base(&token, &cfg.github.api_base, &owner, &repo, pr.number, &base)?;
                pb.finish_and_clear();
                Ok(updated)
            } else {
                pb.finish_and_clear();
                Ok(pr)
            }
        } else {
            let pr_title = gitcmd::git_output_lossy(&["log", "--format=%s", "-1", &branch]);
            let title = if pr_title.is_empty() { branch.clone() } else { pr_title };
            let pr = github::create_pr(
                &token, &cfg.github.api_base, &owner, &repo, &title, &branch, &base, draft,
            )?;
            pb.finish_and_clear();
            Ok(pr)
        };

        match result {
            Ok(pr) => {
                let action = if stack.branches[i].pr_number.is_some() {
                    "Updated"
                } else {
                    "Created"
                };
                ui::print_success(&format!(
                    "{} PR #{}: {} → {}  {}",
                    action,
                    pr.number.to_string().yellow(),
                    branch.green().bold(),
                    base.cyan(),
                    pr.html_url.bright_black().underline()
                ));

                // Store PR info back in stack
                let stack_mut = store.stacks.iter_mut().find(|s| s.name == stack.name);
                if let Some(s) = stack_mut {
                    if let Some(b) = s.branches.iter_mut().find(|b| b.name == branch) {
                        b.pr_number = Some(pr.number);
                        b.pr_url = Some(pr.html_url.clone());
                    }
                }

                if open {
                    let _ = open_url(&pr.html_url);
                }
            }
            Err(e) => {
                ui::print_error(&format!("Failed to create PR for {}: {}", branch.red(), e));
            }
        }
    }

    save_store(&store)?;
    println!();
    Ok(())
}

pub fn remove_branch(branch: &str) -> Result<()> {
    let mut store = load_store()?;

    // Find which stack owns this branch
    let stack_name = store
        .branch_to_stack
        .get(branch)
        .cloned()
        .with_context(|| format!("Branch '{}' is not part of any stack", branch))?;

    let stack = store
        .stacks
        .iter_mut()
        .find(|s| s.name == stack_name)
        .with_context(|| format!("Stack '{}' not found", stack_name))?;

    let pos = stack
        .branches
        .iter()
        .position(|b| b.name == branch)
        .with_context(|| format!("Branch '{}' not in stack", branch))?;

    stack.branches.remove(pos);
    store.branch_to_stack.remove(branch);
    save_store(&store)?;

    ui::print_success(&format!("Removed '{}' from stack", branch.yellow()));
    Ok(())
}

pub fn delete_stack(name: &str, delete_branches: bool) -> Result<()> {
    let mut store = load_store()?;

    let stack = store
        .stacks
        .iter()
        .find(|s| s.name == name)
        .cloned()
        .with_context(|| format!("Stack '{}' not found.", name))?;

    if delete_branches {
        for branch in &stack.branches {
            if let Err(e) = gitcmd::git_output(&["branch", "-d", &branch.name]) {
                ui::print_warning(&format!("Could not delete branch '{}': {}", branch.name, e));
            }
        }
    }

    for branch in &stack.branches {
        store.branch_to_stack.remove(&branch.name);
    }
    store.stacks.retain(|s| s.name != name);
    save_store(&store)?;

    ui::print_success(&format!("Deleted stack '{}'", name.red()));
    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn get_github_token(cfg: &config::Config) -> Result<String> {
    // Prefer GITHUB_TOKEN env var
    if let Ok(t) = std::env::var("GITHUB_TOKEN") {
        return Ok(t);
    }
    cfg.github
        .token
        .clone()
        .filter(|t| !t.is_empty())
        .with_context(|| {
            "GitHub token not found. Set GITHUB_TOKEN env var or add `token` to [github] in ~/.config/vcli/config.toml".to_string()
        })
}

fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(url).spawn()?;
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open").arg(url).spawn()?;
    #[cfg(target_os = "windows")]
    std::process::Command::new("cmd").args(["/C", "start", url]).spawn()?;
    Ok(())
}
