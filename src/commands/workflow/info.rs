//! `g workflow info <name>` — show detailed workflow information with diagram.

use anyhow::Result;

use crate::cli::workflow::InfoArgs;
use crate::commands::workflow::shared::get_workflow;
use crate::config::workflow_presets;
use crate::commands::Ctx;
use crate::ui::{muted, primary_bold};

pub fn run(_ctx: &Ctx, args: InfoArgs) -> Result<()> {
    let workflow = get_workflow(&args.name)?;
    let docs = workflow_presets::get_docs(&args.name);

    println!();

    // Header with name
    let title = docs
        .map(|d| format_title(d.name))
        .unwrap_or_else(|| args.name.clone());

    print_box_top(&title);

    // Diagram
    if let Some(d) = docs {
        println!("{}", d.diagram);
        print_box_separator();

        // Description
        println!(" {}", d.description);
        println!();

        // Use cases
        println!(" Use Cases");
        for case in d.use_cases {
            println!("   * {}", case);
        }
        println!();

        // Pros and Cons side by side
        let max_pro_len = d.pros.iter().map(|s| s.len()).max().unwrap_or(0);
        let col_width = max_pro_len + 8;

        println!(" {:width$} | Cons", "Pros", width = col_width - 3);
        println!(" {:-<width$}-+-{:-<30}", "", "", width = col_width - 3);

        let max_rows = std::cmp::max(d.pros.len(), d.cons.len());
        for i in 0..max_rows {
            let pro = d.pros.get(i).map(|s| format!("+ {}", s)).unwrap_or_default();
            let con = d.cons.get(i).map(|s| format!("- {}", s)).unwrap_or_default();
            println!(" {:width$} | {}", pro, con, width = col_width - 3);
        }

        print_box_separator();
    }

    // Branch Types
    println!(" Branch Types");
    println!();

    for bt in &workflow.types {
        let prefix_display = if bt.prefix.is_empty() {
            "(no prefix)".to_string()
        } else {
            bt.prefix.clone()
        };

        println!(" {} {}", primary_bold(&bt.name), muted(&prefix_display));

        // Source and target
        let source = workflow.effective_source(bt);
        let target = workflow
            .effective_target(bt)
            .map(|t| t.to_string())
            .unwrap_or_else(|| "(none)".to_string());

        println!("   Source: {}    Target: {}", source, target);
        println!("   Strategy: {}", bt.merge_strategy);

        // Options
        let mut options = Vec::new();
        if bt.delete_after_merge == Some(true) {
            options.push("delete after merge");
        }
        if bt.require_pr == Some(true) {
            options.push("require PR");
        }
        if bt.tag_on_finish == Some(true) {
            options.push("tag on finish");
        }
        if bt.ephemeral == Some(true) {
            options.push("ephemeral");
        }
        if bt.require_ticket == Some(true) {
            options.push("require ticket");
        }

        if !options.is_empty() {
            println!("   Options: {}", options.join(", "));
        }

        println!();
    }

    // Configuration
    print_box_separator();
    println!(" Configuration");
    println!();

    println!("   Main branch:    {}", workflow.main_branch);
    if let Some(ref develop) = workflow.develop_branch {
        println!("   Develop branch: {}", develop);
    }
    if let Some(ref versions) = workflow.supported_versions {
        println!("   LTS versions:   {}", versions.join(", "));
    }
    if let Some(ref pattern) = workflow.ticket_pattern {
        println!("   Ticket pattern: {}", pattern);
    }

    // Rules
    if let Some(ref rules) = workflow.rules {
        println!();
        println!("   Rules:");
        if rules.require_clean_tree == Some(true) {
            println!("     - Require clean working tree");
        }
        if rules.require_up_to_date == Some(true) {
            println!("     - Require source branch up-to-date");
        }
        if let Some(days) = rules.max_branch_age_days {
            println!("     - Warn after {} days", days);
        }
        if rules.require_feature_flags == Some(true) {
            println!("     - Encourage feature flags (advisory)");
        }
    }

    // Hooks
    if let Some(ref hooks) = workflow.hooks {
        if !hooks.is_empty() {
            println!();
            println!("   Hooks:");
            if let Some(ref cmds) = hooks.pre_start {
                if !cmds.is_empty() {
                    println!("     pre_start: {} command(s)", cmds.len());
                }
            }
            if let Some(ref cmds) = hooks.post_start {
                if !cmds.is_empty() {
                    println!("     post_start: {} command(s)", cmds.len());
                }
            }
            if let Some(ref cmds) = hooks.pre_finish {
                if !cmds.is_empty() {
                    println!("     pre_finish: {} command(s)", cmds.len());
                }
            }
            if let Some(ref cmds) = hooks.post_finish {
                if !cmds.is_empty() {
                    println!("     post_finish: {} command(s)", cmds.len());
                }
            }
            if let Some(ref cmds) = hooks.on_publish {
                if !cmds.is_empty() {
                    println!("     on_publish: {} command(s)", cmds.len());
                }
            }
        }
    }

    print_box_bottom();
    println!();

    Ok(())
}

fn format_title(name: &str) -> String {
    // Capitalize and format name
    name.chars()
        .enumerate()
        .map(|(i, c)| {
            if i == 0 || name.chars().nth(i - 1) == Some('-') {
                c.to_uppercase().next().unwrap_or(c)
            } else if c == '-' {
                ' '
            } else {
                c
            }
        })
        .collect()
}

fn print_box_top(title: &str) {
    let width = 72;
    let padding = (width - title.len() - 4) / 2;
    let left_pad = " ".repeat(padding);
    let right_pad = " ".repeat(width - title.len() - padding - 4);

    println!("+{}+", "-".repeat(width - 2));
    println!("|{}{}{}|", left_pad, title, right_pad);
    println!("+{}+", "-".repeat(width - 2));
}

fn print_box_separator() {
    println!("+{}+", "-".repeat(70));
}

fn print_box_bottom() {
    println!("+{}+", "-".repeat(70));
}
