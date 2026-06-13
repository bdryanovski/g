//! Dry-run path for `g commit`: prints the planned `git commit` invocation
//! without executing it.

use crate::cli::CommitArgs;
use crate::commands::git as gitcmd;
use crate::config;
use crate::ui;
use anyhow::Result;

use super::parse::message_from_flags;

/// Show what the commit command would do in dry-run mode without executing it.
pub(super) fn commit_dry_run(args: &CommitArgs, cfg: &config::Config) -> Result<()> {
    if args.all {
        gitcmd::git_mutate(&["add", "-A"], "Stage all tracked and untracked files")?;
    }

    let message_desc = message_from_flags(args)
        .unwrap_or_else(|| "<interactive prompt — message built via guided flow>".to_string());

    let mut git_args: Vec<&str> = vec!["commit", "-m"];
    // We must keep `msg_placeholder` alive for the lifetime of `git_args`.
    let msg_placeholder;
    if args.message.is_some() {
        msg_placeholder = message_desc.clone();
        git_args.push(&msg_placeholder);
    } else {
        git_args.push("<message>");
    }

    if args.no_verify {
        git_args.push("--no-verify");
    }
    if args.amend {
        git_args.push("--amend");
    }
    if cfg.commit.sign_off {
        git_args.push("--signoff");
    }
    if cfg.commit.gpg_sign {
        git_args.push("--gpg-sign");
    }

    let explanation = if args.amend {
        "Amend the previous commit with staged changes"
    } else {
        "Create a new commit with staged changes"
    };

    gitcmd::git_mutate(&git_args, explanation)?;

    if args.message.is_some() {
        ui::print_line(&format!(
            "           {} {}",
            ui::muted("message:"),
            ui::muted(&message_desc)
        ));
    } else {
        ui::print_line(&format!(
            "           {} {}",
            ui::muted("note:"),
            ui::muted("Commit message would be built via interactive prompts")
        ));
    }

    Ok(())
}
