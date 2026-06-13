//! Special mode: `g stats --search "QUERY"` — fuzzy-search across commit
//! messages that have been imported via [`super::import`].

use anyhow::Result;

use crate::commands::Ctx;
use crate::storage::stats as db;
use crate::ui;
use crate::ui::indent;

pub(super) fn run(ctx: &Ctx, query: &str) -> Result<()> {
    let conn = ctx.conn;
    ui::print_blank();
    ui::print_fieldset(&format!("Search: \"{}\"", query));
    ui::print_blank();

    let results = db::search_commits(conn, query, 25)?;

    if results.is_empty() {
        ui::print_info("No commits found matching your query.");
        ui::print_tip("Try importing git history first with: g stats --import");
        ui::print_blank();
        return Ok(());
    }

    for result in &results {
        let short_hash = &result.commit_hash[..7.min(result.commit_hash.len())];
        let author = result.author_name.as_deref().unwrap_or("Unknown");
        let date = &result.committed_at[..10.min(result.committed_at.len())];

        println!(
            "{}{}  {}  {}  {}",
            indent(),
            ui::warning_bold(short_hash),
            ui::muted(date),
            ui::muted(author),
            ui::muted(&format!("[{}]", result.repo_name)),
        );
        println!("{}  {}", indent(), ui::color_subject(&result.subject));

        if let Some(ref body) = result.body {
            let preview: String = body
                .lines()
                .take(2)
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if !preview.is_empty() {
                let truncated = if preview.len() > 80 {
                    format!("{}...", &preview[..77])
                } else {
                    preview
                };
                println!("{}  {}", indent(), ui::muted(&truncated));
            }
        }
        println!();
    }

    ui::print_tip(&format!("Found {} matching commits", results.len()));
    ui::print_blank();
    Ok(())
}
