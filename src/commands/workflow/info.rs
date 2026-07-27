//! `g workflow info <name>` — show detailed workflow information with diagram.

use anyhow::Result;

use crate::cli::workflow::InfoArgs;
use crate::commands::workflow::shared::get_workflow;
use crate::commands::Ctx;
use crate::config::workflow_presets;
use crate::ui::{danger, muted, primary_bold, success, text_bold, Diagram, Panel};

pub fn run(_ctx: &Ctx, args: InfoArgs) -> Result<()> {
    let workflow = get_workflow(&args.name)?;
    let docs = workflow_presets::get_docs(&args.name);

    println!();

    // Header with name
    let title = docs
        .map(|d| format_title(d.name))
        .unwrap_or_else(|| format_title(&args.name));

    let panel = Panel::new().title(&title);
    panel.print_header();

    // Diagram and docs section
    if let Some(d) = docs {
        // Diagram
        Diagram::print_in_panel(&panel, d.diagram);

        panel.print_divider();
        panel.print_empty();

        // Description - word wrap
        for line in wrap_to_width(d.description, panel.inner_width() - 2) {
            panel.print_line(&format!(" {}", line));
        }

        panel.print_empty();
        panel.print_divider();
        panel.print_empty();

        // Use cases
        panel.print_line(&format!(" {}", text_bold("Use Cases")));
        panel.print_empty();
        for case in d.use_cases {
            panel.print_line(&format!("   {} {}", muted("•"), case));
        }

        panel.print_empty();
        panel.print_divider();
        panel.print_empty();

        // Pros and Cons
        print_pros_cons_in_panel(&panel, d.pros, d.cons);

        panel.print_empty();
        panel.print_divider();
    }

    panel.print_empty();

    // Branch Types
    panel.print_line(&format!(" {}", text_bold("Branch Types")));
    panel.print_empty();

    for bt in &workflow.types {
        let prefix_display = if bt.prefix.is_empty() {
            muted("(any branch)")
        } else {
            muted(&format!("{}*", bt.prefix))
        };

        panel.print_line(&format!("   {} {}", primary_bold(&bt.name), prefix_display));

        // Source and target
        let source = workflow.effective_source(bt);
        let target = workflow
            .effective_target(bt)
            .map(|t| t.to_string())
            .unwrap_or_else(|| muted("(none)"));

        panel.print_line(&format!(
            "   {} {}  {} {}  {} {}",
            muted("from:"),
            source,
            muted("→"),
            target,
            muted("via:"),
            bt.merge_strategy
        ));

        // Options as badges
        let mut options = Vec::new();
        if bt.delete_after_merge == Some(true) {
            options.push("delete");
        }
        if bt.require_pr == Some(true) {
            options.push("PR");
        }
        if bt.tag_on_finish == Some(true) {
            options.push("tag");
        }
        if bt.ephemeral == Some(true) {
            options.push("ephemeral");
        }
        if bt.require_ticket == Some(true) {
            options.push("ticket");
        }

        if !options.is_empty() {
            let badges: String = options
                .iter()
                .map(|o| format!("[{}]", muted(o)))
                .collect::<Vec<_>>()
                .join(" ");
            panel.print_line(&format!("   {}", badges));
        }

        panel.print_empty();
    }

    panel.print_divider();
    panel.print_empty();

    // Configuration
    panel.print_line(&format!(" {}", text_bold("Configuration")));
    panel.print_empty();

    panel.print_line(&format!("   {} {}", muted("main:"), workflow.main_branch));
    if let Some(ref develop) = workflow.develop_branch {
        panel.print_line(&format!("   {} {}", muted("develop:"), develop));
    }
    if let Some(ref versions) = workflow.supported_versions {
        panel.print_line(&format!("   {} {}", muted("LTS:"), versions.join(", ")));
    }
    if let Some(ref pattern) = workflow.ticket_pattern {
        panel.print_line(&format!("   {} {}", muted("ticket:"), pattern));
    }

    // Rules
    if let Some(ref rules) = workflow.rules {
        panel.print_empty();
        panel.print_line(&format!("   {}", muted("Rules:")));

        if rules.require_clean_tree == Some(true) {
            panel.print_line(&format!("     {} Require clean working tree", muted("•")));
        }
        if rules.require_up_to_date == Some(true) {
            panel.print_line(&format!(
                "     {} Require source branch up-to-date",
                muted("•")
            ));
        }
        if let Some(days) = rules.max_branch_age_days {
            panel.print_line(&format!("     {} Warn after {} days", muted("•"), days));
        }
        if rules.require_feature_flags == Some(true) {
            panel.print_line(&format!("     {} Encourage feature flags", muted("•")));
        }
    }

    // Hooks
    if let Some(ref hooks) = workflow.hooks {
        let hook_count = count_hooks(hooks);
        if hook_count > 0 {
            panel.print_empty();
            panel.print_line(&format!(
                "   {} {} hook(s) configured",
                muted("Hooks:"),
                hook_count
            ));
        }
    }

    panel.print_empty();
    panel.print_footer();
    println!();

    Ok(())
}

fn format_title(name: &str) -> String {
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

fn wrap_to_width(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut current = String::new();

    for word in words {
        if current.is_empty() {
            current = word.to_string();
        } else if current.len() + 1 + word.len() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            current = word.to_string();
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn print_pros_cons_in_panel(panel: &Panel, pros: &[&str], cons: &[&str]) {
    // Simple two-column layout
    panel.print_line(&format!(" {}", text_bold("Pros")));
    for pro in pros {
        panel.print_line(&format!("   {} {}", success("+"), pro));
    }
    panel.print_empty();
    panel.print_line(&format!(" {}", text_bold("Cons")));
    for con in cons {
        panel.print_line(&format!("   {} {}", danger("-"), con));
    }
}

fn count_hooks(hooks: &crate::config::workflow::WorkflowHooks) -> usize {
    let mut count = 0;
    if let Some(ref cmds) = hooks.pre_start {
        count += cmds.len();
    }
    if let Some(ref cmds) = hooks.post_start {
        count += cmds.len();
    }
    if let Some(ref cmds) = hooks.pre_finish {
        count += cmds.len();
    }
    if let Some(ref cmds) = hooks.post_finish {
        count += cmds.len();
    }
    if let Some(ref cmds) = hooks.on_publish {
        count += cmds.len();
    }
    count
}
