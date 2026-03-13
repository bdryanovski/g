use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::commands::git as gitcmd;
use crate::config;
use crate::ui;

// ─── Data Structures ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Workspace {
    pub name: String,
    pub description: Option<String>,
    pub branch: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Map of filename → content hash (for quick checking)
    pub env_files: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct WorkspaceStore {
    pub workspaces: Vec<Workspace>,
    pub current: Option<String>,
}

// ─── Persistence ─────────────────────────────────────────────────────────────

fn load_store() -> Result<WorkspaceStore> {
    let path = config::workspaces_path()?;
    if !path.exists() {
        return Ok(WorkspaceStore::default());
    }
    let raw = fs::read_to_string(&path).context("Failed to read workspaces file")?;
    toml::from_str(&raw).context("Failed to parse workspaces file")
}

fn save_store(store: &WorkspaceStore) -> Result<()> {
    let path = config::workspaces_path()?;
    let raw = toml::to_string_pretty(store).context("Failed to serialize workspaces")?;
    fs::write(&path, raw).context("Failed to save workspaces file")
}

fn workspace_files_dir(name: &str) -> Result<PathBuf> {
    let dir = config::workspace_store_dir()?.join(name);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

// ─── Commands ─────────────────────────────────────────────────────────────────

pub fn list() -> Result<()> {
    let store = load_store()?;

    if store.workspaces.is_empty() {
        println!();
        println!("  {}", "No workspaces yet.".bright_black());
        println!("  {} {}", "tip:".bright_black(), "vcli workspace create <name>  to create one".bright_black());
        println!();
        return Ok(());
    }

    println!();
    let mut table = ui::Table::new(vec!["", "Name", "Branch", "Env Files", "Updated"]);
    for ws in &store.workspaces {
        let is_current = store.current.as_deref() == Some(&ws.name);
        let marker = if is_current { "◉".green().bold().to_string() } else { "◯".bright_black().to_string() };
        let name = if is_current {
            ws.name.green().bold().to_string()
        } else {
            ws.name.white().to_string()
        };
        let desc = ws.description.as_deref().unwrap_or("");
        let label = if desc.is_empty() {
            name
        } else {
            format!("{}  {}", name, desc.bright_black())
        };
        table.add_row(vec![
            marker,
            label,
            ui::color_branch(&ws.branch),
            format!("{} files", ws.env_files.len()).bright_black().to_string(),
            format_relative_time(ws.updated_at),
        ]);
    }
    table.print();
    println!();
    Ok(())
}

pub fn create(name: &str, description: Option<&str>) -> Result<()> {
    let mut store = load_store()?;
    let cfg = config::load()?;

    if store.workspaces.iter().any(|w| w.name == name) {
        bail!("Workspace '{}' already exists. Use a different name.", name);
    }

    let branch = gitcmd::current_branch().context("Not inside a git repository")?;
    let repo_root = gitcmd::repo_root()?;
    let dest_dir = workspace_files_dir(name)?;

    // Copy env files
    let mut env_files = HashMap::new();
    let mut copied = 0usize;

    for pattern in &cfg.workspace.copy_patterns {
        let matches = glob_files(&repo_root, pattern);
        for file_path in matches {
            let rel = file_path
                .strip_prefix(&repo_root)
                .unwrap_or(&file_path)
                .to_string_lossy()
                .to_string();
            let dest = dest_dir.join(&rel);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            if let Ok(content) = fs::read_to_string(&file_path) {
                let hash = simple_hash(&content);
                fs::write(&dest, &content)?;
                env_files.insert(rel, hash);
                copied += 1;
            }
        }
    }

    let now = Utc::now();
    store.workspaces.push(Workspace {
        name: name.to_string(),
        description: description.map(str::to_string),
        branch: branch.clone(),
        created_at: now,
        updated_at: now,
        env_files,
    });

    if store.current.is_none() {
        store.current = Some(name.to_string());
    }

    save_store(&store)?;

    println!();
    ui::print_success(&format!(
        "Created workspace {} on branch {}",
        name.green().bold(),
        branch.cyan()
    ));
    if copied > 0 {
        println!(
            "  {} Captured {} env file{}",
            "  ".bright_black(),
            copied.to_string().yellow(),
            if copied == 1 { "" } else { "s" }
        );
    }
    println!();
    Ok(())
}

pub fn switch(name: &str, no_stash: bool) -> Result<()> {
    let mut store = load_store()?;
    let cfg = config::load()?;

    let target_name = store
        .workspaces
        .iter()
        .find(|w| w.name == name || w.name.contains(name))
        .map(|w| w.name.clone())
        .with_context(|| format!("Workspace '{}' not found. Run `vcli workspace list` to see all workspaces.", name))?;

    let current_branch = gitcmd::current_branch()?;

    // Check for uncommitted changes
    let has_changes = !gitcmd::git_output_lossy(&["status", "--porcelain"]).is_empty();

    if has_changes {
        if cfg.workspace.auto_stash && !no_stash {
            let pb = ui::spinner(&format!("Stashing changes on {}", current_branch));
            gitcmd::git_output(&["stash", "push", "-m", &format!("vcli-auto-stash-{}", current_branch)])?;
            pb.finish_and_clear();
            ui::print_info(&format!("Stashed changes on {}", current_branch.cyan()));
        } else if !no_stash {
            bail!(
                "You have uncommitted changes. Use --no-stash to switch anyway (changes will remain), or commit/stash first."
            );
        }
    }

    let repo_root = gitcmd::repo_root()?;

    // Save the current workspace's env files before switching
    if let Some(current_name) = &store.current {
        if let Some(current_ws) = store.workspaces.iter_mut().find(|w| &w.name == current_name) {
            let save_dir = workspace_files_dir(&current_ws.name)?;
            for (rel_path, hash) in current_ws.env_files.iter_mut() {
                let src = Path::new(&repo_root).join(rel_path);
                if src.exists() {
                    if let Ok(content) = fs::read_to_string(&src) {
                        let new_hash = simple_hash(&content);
                        if new_hash != *hash {
                            let dest = save_dir.join(rel_path);
                            if let Some(parent) = dest.parent() {
                                fs::create_dir_all(parent)?;
                            }
                            fs::write(&dest, &content)?;
                            *hash = new_hash;
                        }
                    }
                }
            }
            current_ws.updated_at = Utc::now();
        }
    }

    // Switch branch
    let target_ws = store
        .workspaces
        .iter()
        .find(|w| w.name == target_name)
        .cloned()
        .unwrap();

    let pb = ui::spinner(&format!("Switching to branch {}", target_ws.branch));
    gitcmd::git_output(&["checkout", &target_ws.branch])
        .with_context(|| format!("Failed to switch to branch '{}'", target_ws.branch))?;
    pb.finish_and_clear();

    // Restore target workspace's env files
    let src_dir = workspace_files_dir(&target_ws.name)?;
    let mut restored = 0usize;

    for (rel_path, _hash) in &target_ws.env_files {
        let src = src_dir.join(rel_path);
        let dest = Path::new(&repo_root).join(rel_path);
        if src.exists() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            if let Ok(content) = fs::read_to_string(&src) {
                fs::write(&dest, content)?;
                restored += 1;
            }
        }
    }

    // Update current workspace and timestamp
    store.current = Some(target_ws.name.clone());
    if let Some(ws) = store.workspaces.iter_mut().find(|w| w.name == target_name) {
        ws.updated_at = Utc::now();
    }
    save_store(&store)?;

    println!();
    ui::print_success(&format!(
        "Switched to workspace {} → branch {}",
        target_ws.name.green().bold(),
        target_ws.branch.cyan().bold()
    ));

    if restored > 0 {
        println!(
            "     {} Restored {} env file{}",
            "".bright_black(),
            restored.to_string().yellow(),
            if restored == 1 { "" } else { "s" }
        );
    }

    if let Some(desc) = &target_ws.description {
        println!("     {} {}", "desc:".bright_black(), desc.bright_white());
    }
    println!();
    Ok(())
}

pub fn delete(name: &str) -> Result<()> {
    let mut store = load_store()?;

    let idx = store
        .workspaces
        .iter()
        .position(|w| w.name == name)
        .with_context(|| format!("Workspace '{}' not found.", name))?;

    store.workspaces.remove(idx);
    if store.current.as_deref() == Some(name) {
        store.current = store.workspaces.first().map(|w| w.name.clone());
    }

    // Remove stored files
    let dir = config::workspace_store_dir()?.join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }

    save_store(&store)?;
    ui::print_success(&format!("Deleted workspace '{}'", name.red()));
    Ok(())
}

pub fn status() -> Result<()> {
    let store = load_store()?;
    let branch = gitcmd::current_branch().unwrap_or_else(|_| "unknown".into());

    println!();
    if let Some(current_name) = &store.current {
        if let Some(ws) = store.workspaces.iter().find(|w| &w.name == current_name) {
            println!(
                "  {} {} {}",
                "Current workspace:".bright_black(),
                ws.name.green().bold(),
                format!("({})", ws.branch).bright_black()
            );
            if let Some(desc) = &ws.description {
                println!("  {} {}", "Description:".bright_black(), desc);
            }
            println!(
                "  {} {}",
                "Env files:".bright_black(),
                ws.env_files.len().to_string().yellow()
            );
            println!(
                "  {} {}",
                "Active branch:".bright_black(),
                branch.cyan().bold()
            );
            if ws.branch != branch {
                ui::print_warning(&format!(
                    "Workspace branch '{}' ≠ current branch '{}'",
                    ws.branch, branch
                ));
            }
        }
    } else {
        println!("  {}", "No active workspace.".bright_black());
    }
    println!();
    Ok(())
}

pub fn rename(old: &str, new: &str) -> Result<()> {
    let mut store = load_store()?;
    let ws = store
        .workspaces
        .iter_mut()
        .find(|w| w.name == old)
        .with_context(|| format!("Workspace '{}' not found.", old))?;

    ws.name = new.to_string();
    if store.current.as_deref() == Some(old) {
        store.current = Some(new.to_string());
    }

    // Rename stored files directory
    let old_dir = config::workspace_store_dir()?.join(old);
    let new_dir = config::workspace_store_dir()?.join(new);
    if old_dir.exists() {
        fs::rename(&old_dir, &new_dir)?;
    }

    save_store(&store)?;
    ui::print_success(&format!("Renamed workspace '{}' → '{}'", old, new.green()));
    Ok(())
}

// ─── Utilities ────────────────────────────────────────────────────────────────

fn glob_files(root: &str, pattern: &str) -> Vec<PathBuf> {
    // Simple pattern matching: supports leading * wildcard
    let root_path = Path::new(root);
    let mut results = vec![];

    if pattern.contains('*') {
        // Split on * and check prefix/suffix
        let parts: Vec<&str> = pattern.splitn(2, '*').collect();
        let prefix = parts[0];
        let suffix = if parts.len() > 1 { parts[1] } else { "" };

        if let Ok(entries) = fs::read_dir(root_path) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                if name.starts_with(prefix) && name.ends_with(suffix) {
                    results.push(entry.path());
                }
            }
        }
    } else {
        let full = root_path.join(pattern);
        if full.exists() {
            results.push(full);
        }
    }

    results
}

fn simple_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    content.hash(&mut h);
    format!("{:x}", h.finish())
}

fn format_relative_time(dt: DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(dt);
    if diff.num_days() > 365 {
        format!("{} years ago", diff.num_days() / 365)
    } else if diff.num_days() > 30 {
        format!("{} months ago", diff.num_days() / 30)
    } else if diff.num_days() > 0 {
        format!("{} days ago", diff.num_days())
    } else if diff.num_hours() > 0 {
        format!("{} hours ago", diff.num_hours())
    } else {
        format!("{} min ago", diff.num_minutes().max(1))
    }
    .bright_black()
    .to_string()
}
