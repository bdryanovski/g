//! Stacked PR workflow management.
//!
//! Stacks are persisted in `stacks.toml`, keyed by repository path so multiple
//! repos can share the same global config directory without collisions.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

use crate::commands::git as gitcmd;
use crate::config;
use crate::github;
use crate::ui;

// ─── Data Structures ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Stack {
    pub name: String,
    pub root: String,
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

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct RepoStacks {
    pub stacks: Vec<Stack>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct StackStore {
    #[serde(default)]
    pub repositories: HashMap<String, RepoStacks>,
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

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn repo_id() -> Result<String> {
    gitcmd::repo_root()
}

fn repo_stacks<'a>(store: &'a StackStore, repo: &str) -> Option<&'a Vec<Stack>> {
    store.repositories.get(repo).map(|r| &r.stacks)
}

fn repo_stacks_mut<'a>(store: &'a mut StackStore, repo: &str) -> &'a mut Vec<Stack> {
    &mut store
        .repositories
        .entry(repo.to_string())
        .or_default()
        .stacks
}

fn current_stack(store: &StackStore) -> Result<&Stack> {
    let repo = repo_id()?;
    let branch = gitcmd::current_branch()?;

    let stacks = repo_stacks(store, &repo).with_context(|| {
        "No stacks in this repository. Use `g stack new <name>` to create one.".to_string()
    })?;

    stacks
        .iter()
        .find(|s| s.branches.iter().any(|b| b.name == branch))
        .with_context(|| {
            format!(
                "Branch '{}' is not part of any stack. Use `g stack new <name>` to create one.",
                branch
            )
        })
}

fn find_stack_for_branch<'a>(stacks: &'a [Stack], branch: &str) -> Option<&'a Stack> {
    stacks
        .iter()
        .find(|s| s.branches.iter().any(|b| b.name == branch))
}

fn get_github_token(cfg: &config::Config) -> Result<String> {
    if let Ok(t) = std::env::var("GITHUB_TOKEN") {
        return Ok(t);
    }
    cfg.github
        .token
        .clone()
        .filter(|t| !t.is_empty())
        .with_context(|| {
            "GitHub token not found. Set GITHUB_TOKEN env var or add `token` to [github] in config."
                .to_string()
        })
}

fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(url).spawn()?;
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open").arg(url).spawn()?;
    #[cfg(target_os = "windows")]
    std::process::Command::new("cmd")
        .args(["/C", "start", url])
        .spawn()?;
    Ok(())
}

// ─── Commands ─────────────────────────────────────────────────────────────────

pub fn new_stack(name: &str) -> Result<()> {
    let mut store = load_store()?;
    let repo = repo_id()?;
    let branch = gitcmd::current_branch()?;

    let stacks = repo_stacks_mut(&mut store, &repo);

    if stacks.iter().any(|s| s.name == name) {
        bail!("Stack '{}' already exists in this repository.", name);
    }

    let now = Utc::now();
    stacks.push(Stack {
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
    });

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

pub fn add_branch(branch_name: &str) -> Result<()> {
    let mut store = load_store()?;
    let repo = repo_id()?;
    let current_branch = gitcmd::current_branch()?;

    let (stack_name, current_pos) = {
        let stack = current_stack(&store)?;
        let pos = stack
            .branches
            .iter()
            .position(|b| b.name == current_branch)
            .with_context(|| format!("Branch '{}' not in stack", current_branch))?;
        (stack.name.clone(), pos)
    };

    gitcmd::git_output(&["checkout", "-b", branch_name])
        .with_context(|| format!("Failed to create branch '{}'", branch_name))?;

    let stacks = repo_stacks_mut(&mut store, &repo);
    let stack = stacks
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
    let repo = repo_id()?;
    let current_branch = gitcmd::current_branch().unwrap_or_default();

    let stacks = match repo_stacks(&store, &repo) {
        Some(s) if !s.is_empty() => s,
        _ => {
            println!();
            println!("  {}", "No stacks yet.".bright_black());
            println!(
                "  {} {}",
                "tip:".bright_black(),
                "g stack new <name>  to create a stack from the current branch".bright_black()
            );
            println!();
            return Ok(());
        }
    };

    for stack in stacks {
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

            print!("{} {} {}", connector.bright_black(), marker, name_colored);

            if let Some(pr_url) = &branch.pr_url {
                let pr_num = branch
                    .pr_number
                    .map(|n| format!(" #{}", n))
                    .unwrap_or_default();
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

pub fn details() -> Result<()> {
    let store = load_store()?;
    let stack = current_stack(&store)?.clone();
    let current_branch = gitcmd::current_branch().unwrap_or_default();

    let open_prs = fetch_open_prs_for_details();

    println!();
    println!(
        "  {} {}  {}",
        "Stack:".bright_black(),
        stack.name.cyan().bold(),
        format!("(root: {})", stack.root).bright_black()
    );
    println!();

    let last_branch = stack.branches.len().saturating_sub(1);

    for (i, branch) in stack.branches.iter().enumerate() {
        let is_current = branch.name == current_branch;
        let connector = if i == last_branch { "└──" } else { "├──" };
        let pipe = if i == last_branch { " " } else { "│" };

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

        print!("  {} {} {}", connector.bright_black(), marker, name_colored);

        if is_current {
            print!("  {}", "(current)".green().dimmed());
        }
        println!();

        let branch_time = gitcmd::git_output_lossy(&["log", "-1", "--format=%ar", &branch.name]);
        if !branch_time.is_empty() {
            println!(
                "  {}     {}",
                pipe.bright_black(),
                branch_time.trim().bright_black()
            );
        }

        let live_pr = open_prs.as_ref().and_then(|prs| prs.get(&branch.name));
        if let Some(pr) = live_pr {
            println!(
                "  {}     {} {}  {}",
                pipe.bright_black(),
                "PR".bright_black(),
                format!("#{}", pr.number).cyan(),
                pr.html_url.bright_black().underline()
            );
        } else if let Some(pr_url) = &branch.pr_url {
            let pr_num = branch
                .pr_number
                .map(|n| format!("#{}", n))
                .unwrap_or_default();
            println!(
                "  {}     {} {}  {}",
                pipe.bright_black(),
                "PR".bright_black(),
                pr_num.cyan(),
                pr_url.bright_black().underline()
            );
        }

        let base = if i == 0 {
            &stack.root
        } else {
            &stack.branches[i - 1].name
        };

        if i > 0 || branch.name != stack.root {
            let range = format!("{}..{}", base, branch.name);
            let commits = gitcmd::git_output_lossy(&[
                "log",
                "--format=%h%x1f%s%x1f%an%x1f%ar",
                "--reverse",
                &range,
            ]);

            if !commits.is_empty() {
                println!("  {}", pipe.bright_black());
                for commit_line in commits.lines() {
                    let parts: Vec<&str> = commit_line.split('\x1f').collect();
                    if parts.len() >= 4 {
                        println!(
                            "  {}     {} - {}  {}",
                            pipe.bright_black(),
                            parts[0].yellow().dimmed(),
                            parts[1].bright_black(),
                            format!("({}, {})", parts[2], parts[3])
                                .bright_black()
                                .dimmed()
                        );
                    } else if let Some((hash, subject)) = commit_line.split_once(' ') {
                        println!(
                            "  {}     {} - {}",
                            pipe.bright_black(),
                            hash.yellow().dimmed(),
                            subject.bright_black()
                        );
                    }
                }
            } else {
                println!(
                    "  {}     {}",
                    pipe.bright_black(),
                    "(no commits)".bright_black()
                );
            }
        }

        if i < last_branch {
            println!("  {}   {}", "│".bright_black(), "│".bright_black());
        }
    }

    println!();
    Ok(())
}

/// Try to fetch open PRs from GitHub for display in stack details.
/// Returns None silently if no token is configured or the API call fails,
/// so `details()` can always run without blocking on network errors.
fn fetch_open_prs_for_details() -> Option<std::collections::HashMap<String, github::PrInfo>> {
    let cfg = config::load().ok()?;
    let token = get_github_token(&cfg).ok()?;
    let (owner, repo_name) = github::detect_repo().ok()?;
    github::list_open_prs(&token, &cfg.github.api_base, &owner, &repo_name).ok()
}

pub fn switch_stack(name: &str) -> Result<()> {
    let store = load_store()?;
    let repo = repo_id()?;

    let stacks = repo_stacks(&store, &repo)
        .with_context(|| "No stacks in this repository.".to_string())?;

    let stack = stacks
        .iter()
        .find(|s| s.name == name || s.name.contains(name))
        .with_context(|| {
            format!(
                "Stack '{}' not found. Run `g stack list` to see all stacks.",
                name
            )
        })?;

    let top_branch = stack
        .branches
        .last()
        .with_context(|| format!("Stack '{}' has no branches.", name))?;

    let current = gitcmd::current_branch().unwrap_or_default();
    if current == top_branch.name {
        println!();
        ui::print_info(&format!(
            "Already on stack {} (branch {})",
            stack.name.cyan().bold(),
            top_branch.name.green().bold()
        ));
        println!();
        return Ok(());
    }

    let pb = ui::spinner(&format!("Switching to stack {}", stack.name.cyan()));
    gitcmd::git_output(&["checkout", &top_branch.name])
        .with_context(|| format!("Failed to checkout branch '{}'", top_branch.name))?;
    pb.finish_and_clear();

    println!();
    ui::print_success(&format!(
        "Switched to stack {} → branch {}",
        stack.name.cyan().bold(),
        top_branch.name.green().bold()
    ));
    println!();
    Ok(())
}

pub fn absorb() -> Result<()> {
    let mut store = load_store()?;
    let repo = repo_id()?;
    let current_branch = gitcmd::current_branch()?;

    let (stack_name, pos) = {
        let stacks = repo_stacks(&store, &repo)
            .with_context(|| "No stacks in this repository.".to_string())?;
        let stack = find_stack_for_branch(stacks, &current_branch)
            .with_context(|| format!("Branch '{}' is not part of any stack.", current_branch))?;
        let pos = stack
            .branches
            .iter()
            .position(|b| b.name == current_branch)
            .unwrap();
        (stack.name.clone(), pos)
    };

    if pos == 0 {
        println!();
        ui::print_warning(
            "This is the bottom branch of the stack — nothing below to absorb into.",
        );
        println!();
        return Ok(());
    }

    let target_branch = {
        let stacks = repo_stacks(&store, &repo).unwrap();
        let stack = stacks.iter().find(|s| s.name == stack_name).unwrap();
        stack.branches[pos - 1].name.clone()
    };
    let absorbed_branch = current_branch.clone();

    println!();
    println!(
        "  {} Merging {} into {}",
        "→".cyan(),
        absorbed_branch.green().bold(),
        target_branch.cyan().bold()
    );

    let pb = ui::spinner(&format!("Checking out {}", target_branch));
    gitcmd::git_output(&["checkout", &target_branch])
        .with_context(|| format!("Failed to checkout '{}'", target_branch))?;
    pb.finish_and_clear();

    let pb = ui::spinner(&format!(
        "Merging {} into {}",
        absorbed_branch, target_branch
    ));
    let merge_result = gitcmd::git_output(&["merge", "--no-ff", &absorbed_branch]);
    pb.finish_and_clear();

    match merge_result {
        Ok(_) => {}
        Err(e) => {
            let msg = format!("{}", e);
            if msg.contains("CONFLICT") || msg.contains("conflict") {
                ui::print_warning("Merge conflicts detected. Resolve them, then:");
                println!("    {} git add <files>", "1.".bright_black());
                println!("    {} git commit", "2.".bright_black());
                println!(
                    "    {} Manually remove '{}' from the stack with: g stack remove {}",
                    "3.".bright_black(),
                    absorbed_branch,
                    absorbed_branch
                );
                println!();
                return Ok(());
            }
            let _ = gitcmd::git_output(&["merge", "--abort"]);
            let _ = gitcmd::git_output(&["checkout", &absorbed_branch]);
            return Err(e).context("Failed to merge branches");
        }
    }

    let _ = gitcmd::git_output(&["branch", "-d", &absorbed_branch]);

    let remaining_count = {
        let stacks = repo_stacks_mut(&mut store, &repo);
        let stack = stacks.iter_mut().find(|s| s.name == stack_name).unwrap();
        stack.branches.remove(pos);
        stack.updated_at = Utc::now();
        stack.branches.len()
    };
    save_store(&store)?;

    ui::print_success(&format!(
        "Absorbed {} into {}",
        absorbed_branch.green().bold(),
        target_branch.cyan().bold()
    ));
    println!(
        "     {} Stack now has {} branch{}",
        "".bright_black(),
        remaining_count.to_string().yellow(),
        if remaining_count == 1 { "" } else { "es" }
    );
    println!();
    Ok(())
}

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

        let pb = ui::spinner(&format!("Rebasing {} onto {}", branch.green(), base.cyan()));

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
                    let _ = gitcmd::git_output(&["rebase", "--abort"]);
                    bail!(
                        "Conflict rebasing '{}' onto '{}': {}\nRun without --no-interactive to resolve manually.",
                        branch, base, e
                    );
                } else {
                    ui::print_warning(&format!(
                        "Conflict in {}: resolve manually, then run `g stack sync` again",
                        branch.yellow()
                    ));
                    println!();
                    println!("  {} After resolving conflicts:", "→".cyan());
                    println!("    {} git add <files>", "1.".bright_black());
                    println!("    {} git rebase --continue", "2.".bright_black());
                    println!(
                        "    {} g stack sync  (to continue remaining branches)",
                        "3.".bright_black()
                    );
                    println!();
                    return Ok(());
                }
            }
        }
    }

    let _ = gitcmd::git_output(&["checkout", &saved_branch]);

    println!();
    ui::print_success("Stack sync complete!");
    println!();
    Ok(())
}

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
                        "g stack push --force".yellow()
                    );
                }
            }
        }
    }

    println!();
    Ok(())
}

pub fn create_prs(open: bool, draft: bool) -> Result<()> {
    let mut store = load_store()?;
    let repo = repo_id()?;
    let stack = current_stack(&store)?.clone();
    let cfg = config::load()?;

    let token = get_github_token(&cfg)?;
    let (owner, repo_name) = github::detect_repo()?;

    println!();
    println!(
        "  {} {} → {}/{}",
        "Creating PRs for stack:".bold().white(),
        stack.name.cyan().bold(),
        owner.bright_white(),
        repo_name.bright_white()
    );
    println!();

    for i in 1..stack.branches.len() {
        let base = stack.branches[i - 1].name.clone();
        let branch = stack.branches[i].name.clone();

        let pb = ui::spinner(&format!(
            "Creating PR: {} → {}",
            branch.green(),
            base.cyan()
        ));

        let existing =
            github::find_pr(&token, &cfg.github.api_base, &owner, &repo_name, &branch)?;

        let result: Result<crate::github::PrInfo, anyhow::Error> = if let Some(pr) = existing {
            if pr.base_ref != base {
                pb.set_message(format!(
                    "Updating PR #{} base: {} → {}",
                    pr.number,
                    pr.base_ref.red(),
                    base.green()
                ));
                let updated = github::update_pr_base(
                    &token,
                    &cfg.github.api_base,
                    &owner,
                    &repo_name,
                    pr.number,
                    &base,
                )?;
                pb.finish_and_clear();
                Ok(updated)
            } else {
                pb.finish_and_clear();
                Ok(pr)
            }
        } else {
            let pr_title = gitcmd::git_output_lossy(&["log", "--format=%s", "-1", &branch]);
            let title = if pr_title.is_empty() {
                branch.clone()
            } else {
                pr_title
            };
            let pr = github::create_pr(
                &token,
                &cfg.github.api_base,
                &owner,
                &repo_name,
                &title,
                &branch,
                &base,
                draft,
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

                let stacks = repo_stacks_mut(&mut store, &repo);
                if let Some(s) = stacks.iter_mut().find(|s| s.name == stack.name) {
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
                ui::print_error(&format!(
                    "Failed to create PR for {}: {}",
                    branch.red(),
                    e
                ));
            }
        }
    }

    save_store(&store)?;
    println!();
    Ok(())
}

pub fn remove_branch(branch: &str) -> Result<()> {
    let mut store = load_store()?;
    let repo = repo_id()?;

    let stacks = repo_stacks_mut(&mut store, &repo);

    let stack = stacks
        .iter_mut()
        .find(|s| s.branches.iter().any(|b| b.name == branch))
        .with_context(|| format!("Branch '{}' is not part of any stack", branch))?;

    let pos = stack
        .branches
        .iter()
        .position(|b| b.name == branch)
        .with_context(|| format!("Branch '{}' not in stack", branch))?;

    stack.branches.remove(pos);
    save_store(&store)?;

    ui::print_success(&format!("Removed '{}' from stack", branch.yellow()));
    Ok(())
}

pub fn delete_stack(name: &str, delete_branches: bool) -> Result<()> {
    let mut store = load_store()?;
    let repo = repo_id()?;

    let stacks = repo_stacks_mut(&mut store, &repo);

    let stack = stacks
        .iter()
        .find(|s| s.name == name)
        .cloned()
        .with_context(|| format!("Stack '{}' not found.", name))?;

    if delete_branches {
        for branch in &stack.branches {
            if let Err(e) = gitcmd::git_output(&["branch", "-d", &branch.name]) {
                ui::print_warning(&format!(
                    "Could not delete branch '{}': {}",
                    branch.name, e
                ));
            }
        }
    }

    stacks.retain(|s| s.name != name);
    save_store(&store)?;

    ui::print_success(&format!("Deleted stack '{}'", name.red()));
    Ok(())
}
