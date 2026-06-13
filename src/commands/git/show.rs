//! `g show` — commit metadata with rich formatting, followed by its diff
//! (rendered through whatever tool [`super::diff::enhanced_diff`] selects).

use anyhow::Result;

use crate::ui;

use super::diff::enhanced_diff;
use super::exec::git_output_lossy;

/// Show a commit's metadata with rich formatting, followed by its diff.
pub fn enhanced_show(extra_args: &[String]) -> Result<()> {
    let rev = extra_args.first().map(|s| s.as_str()).unwrap_or("HEAD");

    let meta_fmt = "%H\x01%h\x01%s\x01%b\x01%an\x01%ae\x01%ai\x01%ar\x01%D\x01%P";
    let meta_raw = git_output_lossy(&["show", "-s", &format!("--format={}", meta_fmt), rev]);

    for line in meta_raw.lines() {
        let fields: Vec<&str> = line.splitn(10, '\x01').collect();
        if fields.len() >= 9 {
            let (_hash, _short_hash, subject, body, author, email, date_iso, date_rel, refs) = (
                fields[0], fields[1], fields[2], fields[3], fields[4], fields[5], fields[6],
                fields[7], fields[8],
            );

            // Fieldset header: "/////  abc1234  //  feat: add …  ////…"
            let short = _short_hash;
            let subject_preview: String = subject.chars().take(45).collect();
            ui::print_blank();
            ui::print_fieldset(&format!("{}  {}", short, subject_preview));
            ui::print_blank();
            ui::print_key_value_pairs(&[
                ("Author", ui::primary(&format!("{} <{}>", author, email))),
                ("Date", ui::muted(&format!("{}  ({})", date_iso, date_rel))),
                (
                    "Refs",
                    if refs.trim().is_empty() {
                        ui::muted("—")
                    } else {
                        ui::format_refs(refs)
                    },
                ),
            ]);
            if !body.trim().is_empty() {
                ui::print_blank();
                for body_line in body.lines() {
                    ui::print_line(&format!("      {}", ui::paint_text(body_line)));
                }
            }
            ui::print_blank();
            break;
        }
    }

    // Show the diff for this single commit.
    //
    // `<rev>^!` is git's shorthand for "<rev>^..<rev>" — "just the changes
    // introduced by this commit".  It works for all commits with a parent.
    // For the initial commit (no parent), we fall back to `--root <rev>` which
    // treats every file as added from nothing.
    let parents_field = meta_raw
        .lines()
        .next()
        .and_then(|l| l.splitn(10, '\x01').nth(9))
        .unwrap_or("")
        .trim();

    let diff_args: Vec<String> = if parents_field.is_empty() {
        // Initial commit — no parent exists; show everything as additions.
        let mut a = vec!["--root".to_string(), rev.to_string()];
        a.extend(extra_args.iter().filter(|&s| s != rev).cloned());
        a
    } else {
        // Normal commit — diff against first parent using the `^!` notation.
        let mut a = vec![format!("{}^!", rev)];
        a.extend(extra_args.iter().filter(|&s| s != rev).cloned());
        a
    };
    enhanced_diff(&diff_args)
}
