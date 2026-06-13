//! `g status` — pretty-print working-tree state using `--porcelain=v2`.
//!
//! Shows staged, unstaged, untracked, and conflicted files in separate
//! sections with colour-coded status codes and Unicode icons, along with
//! ahead/behind counts for the current tracking branch.

use anyhow::Result;

use crate::ui;

use super::exec::git_output_lossy;
use super::repo::current_branch;

/// Pretty-print git status using `--porcelain=v2` machine-readable output.
pub fn enhanced_status(_extra_args: &[String]) -> Result<()> {
    let branch = current_branch().unwrap_or_else(|_| "unknown".into());

    let raw = git_output_lossy(&["status", "--porcelain=v2", "--branch", "--ahead-behind"]);

    let mut ahead: usize = 0;
    let mut behind: usize = 0;
    let mut upstream: Option<String> = None;

    let mut staged: Vec<(String, String)> = vec![];
    let mut unstaged: Vec<(String, String)> = vec![];
    let mut untracked: Vec<String> = vec![];
    let mut unmerged: Vec<(String, String)> = vec![];

    for line in raw.lines() {
        if line.starts_with("# branch.head ") {
            // Already captured in `current_branch()` above.
        } else if let Some(up) = line.strip_prefix("# branch.upstream ") {
            upstream = Some(up.to_string());
        } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
            // "# branch.ab +3 -1" — ahead/behind counts.
            let parts: Vec<&str> = ab.split_whitespace().collect();
            if parts.len() >= 2 {
                ahead = parts[0].trim_start_matches('+').parse().unwrap_or(0);
                behind = parts[1].trim_start_matches('-').parse().unwrap_or(0);
            }
        } else if let Some(rest) = line.strip_prefix("1 ") {
            // Ordinary changed file: "1 XY sub mH mI mW hH hI path"
            let xy = &rest[..2];
            let fields: Vec<&str> = rest.splitn(9, ' ').collect();
            let path = if fields.len() >= 9 {
                fields[8]
            } else {
                rest.splitn(9, ' ').last().unwrap_or("")
            };
            let x = &xy[0..1]; // staged status
            let y = &xy[1..2]; // unstaged status
            if x != "." {
                staged.push((x.to_string(), path.to_string()));
            }
            if y != "." {
                unstaged.push((y.to_string(), path.to_string()));
            }
        } else if let Some(rest) = line.strip_prefix("2 ") {
            // Renamed or copied file.
            let xy = &rest[..2];
            let fields: Vec<&str> = rest.splitn(10, ' ').collect();
            let paths = if fields.len() >= 10 {
                let p = fields[9];
                if p.contains('\t') {
                    p.split('\t').next().unwrap_or(p).to_string()
                } else {
                    p.to_string()
                }
            } else {
                rest[10..].to_string()
            };
            let x = &xy[0..1];
            let y = &xy[1..2];
            if x != "." {
                staged.push((x.to_string(), paths.clone()));
            }
            if y != "." {
                unstaged.push((y.to_string(), paths));
            }
        } else if let Some(rest) = line.strip_prefix("u ") {
            // Unmerged (conflict) entry.
            let fields: Vec<&str> = rest.splitn(12, ' ').collect();
            let path = fields.last().copied().unwrap_or("").to_string();
            unmerged.push((rest[..2].to_string(), path));
        } else if let Some(rest) = line.strip_prefix("? ") {
            // Untracked file.
            untracked.push(rest.to_string());
        }
    }

    // ─── Print output ─────────────────────────────────────────────────────────

    ui::print_blank();
    print!("  {} {}", ui::muted("On branch"), ui::success_bold(&branch));
    if let Some(up) = &upstream {
        print!("  {}", ui::muted(&format!("tracking {}", up)));
    }
    ui::print_blank();

    if ahead > 0 || behind > 0 {
        ui::print_indented(&ui::format_ahead_behind(ahead, behind));
    }

    if staged.is_empty() && unstaged.is_empty() && untracked.is_empty() && unmerged.is_empty() {
        ui::print_blank();
        ui::print_indented(&format!(
            "{} {}",
            ui::success_bold("✓"),
            ui::success("Working tree is clean")
        ));
        ui::print_blank();
        return Ok(());
    }

    if !staged.is_empty() {
        ui::print_section("Staged Changes", Some(staged.len()));
        let last = staged.len() - 1;
        for (i, (code, path)) in staged.iter().enumerate() {
            let connector = ui::muted(if i == last { "└" } else { "├" });
            let (icon, code_colored) = ui::status_icon(code);
            ui::print_indented(&format!(
                "{} {} {} {}",
                connector,
                code_colored,
                icon,
                ui::success(path)
            ));
        }
    }

    if !unstaged.is_empty() {
        ui::print_section("Unstaged Changes", Some(unstaged.len()));
        let last = unstaged.len() - 1;
        for (i, (code, path)) in unstaged.iter().enumerate() {
            let connector = ui::muted(if i == last { "└" } else { "├" });
            let (icon, code_colored) = ui::status_icon(code);
            ui::print_indented(&format!(
                "{} {} {} {}",
                connector,
                code_colored,
                icon,
                ui::warning(path)
            ));
        }
    }

    if !untracked.is_empty() {
        ui::print_section("Untracked Files", Some(untracked.len()));
        let last = untracked.len() - 1;
        for (i, path) in untracked.iter().enumerate() {
            let connector = ui::muted(if i == last { "└" } else { "├" });
            ui::print_indented(&format!(
                "{} {} {}",
                connector,
                ui::muted("?"),
                ui::muted(path)
            ));
        }
    }

    if !unmerged.is_empty() {
        ui::print_section("Conflicts", Some(unmerged.len()));
        for (code, path) in &unmerged {
            let (icon, _) = ui::status_icon(code);
            ui::print_indented(&format!(
                "{} {} {}",
                ui::danger_bold("  ⚡"),
                icon,
                ui::danger_bold(path)
            ));
        }
    }

    ui::print_blank();

    if !staged.is_empty() {
        ui::print_tip(&format!(
            "{}  commit staged changes",
            ui::warning(&format!("{} commit", crate::bin_name()))
        ));
    } else if !unstaged.is_empty() || !untracked.is_empty() {
        ui::print_tip("git add <file>  or  git add -A  to stage");
    }
    ui::print_blank();

    Ok(())
}
