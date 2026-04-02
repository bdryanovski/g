//! One-time import of legacy TOML data into SQLite.
//!
//! These functions are called from [`super::db::open`] when `workspaces.toml`
//! or `stacks.toml` still exist on disk.  After a successful import the caller
//! renames each file to `.bak`.
//!
//! The TOML serde types are duplicated here (as private structs) so that the
//! `commands/workspace.rs` and `commands/stack.rs` modules can have their own
//! copies removed in a follow-up step without breaking the import path.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use super::{repos, stacks, workspaces};

// ─── Workspace TOML types (private, kept for import only) ────────────────────

#[derive(Debug, Serialize, Deserialize, Default)]
struct WorkspaceStore {
    #[serde(default)]
    repos: HashMap<String, RepoStore>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
struct RepoStore {
    #[serde(default)]
    workspaces: Vec<TomlWorkspace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    container_root: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TomlWorkspace {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    path: String,
    branch: String,
    created_at: DateTime<Utc>,
}

// ─── Stack TOML types (private, kept for import only) ────────────────────────

#[derive(Debug, Serialize, Deserialize, Default)]
struct StackStore {
    #[serde(default)]
    repositories: HashMap<String, RepoStacks>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
struct RepoStacks {
    #[serde(default)]
    stacks: Vec<TomlStack>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TomlStack {
    name: String,
    root: String,
    #[serde(default)]
    branches: Vec<TomlStackBranch>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TomlStackBranch {
    name: String,
    #[serde(default)]
    pr_number: Option<u64>,
    #[serde(default)]
    pr_url: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

// ─── Public import functions ──────────────────────────────────────────────────

/// Import all workspaces from `path` (a `workspaces.toml` file) into SQLite.
///
/// Returns the total number of workspace rows inserted.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed, or if any SQL fails.
pub(super) fn import_workspaces(conn: &Connection, path: &Path) -> Result<usize> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let store: WorkspaceStore =
        toml::from_str(&raw).with_context(|| format!("Failed to parse {}", path.display()))?;

    let mut total = 0usize;

    for (repo_path, repo_store) in &store.repos {
        let repo_id = repos::upsert(conn, repo_path)?;

        // Set container_root on the repo's workspaces if present.
        if let Some(ref root) = repo_store.container_root {
            // We'll apply this after inserting workspaces.
            // Store in a local variable for use below.
            let _ = root; // placeholder — handled after inserts
        }

        for ws in &repo_store.workspaces {
            let new_ws = workspaces::NewWorkspace {
                name: &ws.name,
                description: ws.description.as_deref(),
                path: &ws.path,
                branch: &ws.branch,
                container_root: repo_store.container_root.as_deref(),
                created_at: ws.created_at,
            };

            // Skip if already present (idempotent import).
            if workspaces::find_by_path(conn, &ws.path)?.is_some() {
                continue;
            }

            workspaces::insert(conn, repo_id, &new_ws)?;
            total += 1;
        }

        // Apply container_root to all rows for this repo.
        if let Some(ref root) = repo_store.container_root {
            workspaces::set_container_root(conn, repo_id, root)?;
        }
    }

    Ok(total)
}

/// Import all stacks from `path` (a `stacks.toml` file) into SQLite.
///
/// Returns the total number of stack rows inserted.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed, or if any SQL fails.
pub(super) fn import_stacks(conn: &Connection, path: &Path) -> Result<usize> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let store: StackStore =
        toml::from_str(&raw).with_context(|| format!("Failed to parse {}", path.display()))?;

    let mut total = 0usize;

    for (repo_path, repo_stacks) in &store.repositories {
        let repo_id = repos::upsert(conn, repo_path)?;

        for ts in &repo_stacks.stacks {
            // Skip if a stack with this name already exists for this repo.
            if stacks::load_by_name(conn, repo_id, &ts.name)?.is_some() {
                continue;
            }

            let stack_id = stacks::insert(conn, repo_id, &ts.name, &ts.root)?;

            let branch_rows: Vec<stacks::StackBranchRow> = ts
                .branches
                .iter()
                .enumerate()
                .map(|(pos, b)| stacks::StackBranchRow {
                    position: pos as i32,
                    name: b.name.clone(),
                    pr_number: b.pr_number,
                    pr_url: b.pr_url.clone(),
                    description: b.description.clone(),
                })
                .collect();

            stacks::set_branches(conn, stack_id, &branch_rows)?;
            total += 1;
        }
    }

    Ok(total)
}
