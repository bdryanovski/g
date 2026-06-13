//! Build script — runs automatically on every `cargo build`.
//!
//! Reads the same [`Cli`] struct that powers the binary's `--help` output and
//! uses it to regenerate three classes of artifact every time anything under
//! `src/cli/` changes:
//!
//! | Artifact | Location | Consumed by |
//! |---|---|---|
//! | Man pages | `man/g.1`, `man/g-commit.1`, … | `man g`, `man g-commit`, … |
//! | Shell completions | `completions/g.{bash,zsh,fish,elvish}` | shell tab-complete |
//! | Astro CLI reference | `docs/src/content/docs/cli-reference.md` | GitHub Pages docs site |
//!
//! **No manual steps required.**  Edit a `///` comment in any `src/cli/*.rs`,
//! run `cargo build`, and all three are regenerated in place.
//!
//! ## Testing man pages (no install needed)
//!
//! ```sh
//! MANPATH="$(pwd)/man" man g
//! MANPATH="$(pwd)/man" man g-workspace-create
//! MANPATH="$(pwd)/man" man g-stack-sync
//! ```
//!
//! ## Installing man pages system-wide (one-time)
//!
//! ```sh
//! # macOS
//! sudo cp man/man1/*.1 /usr/local/share/man/man1/
//! sudo /usr/libexec/makewhatis /usr/local/share/man
//!
//! # Linux (Debian/Ubuntu/Arch)
//! sudo cp man/man1/*.1 /usr/local/share/man/man1/
//! sudo mandb
//! ```
//!
//! ## Installing shell completions (one-time)
//!
//! ```sh
//! # zsh
//! cp completions/_g ~/.zsh/completions/_g   # zsh writes `_g` not `g.zsh`
//!
//! # bash
//! cp completions/g.bash ~/.bash_completion
//!
//! # fish
//! cp completions/g.fish ~/.config/fish/completions/g.fish
//! ```

// Pull the CLI definition into this build script so we can call
// `Cli::command()` without duplicating the struct definitions.
// `clap`, `clap_complete`, `clap_mangen`, and `clap-markdown` are all
// declared as [build-dependencies] in Cargo.toml.
//
// `dead_code` is suppressed here because items like `Git(Vec<String>)` and
// `print_completions` are only called by the main binary at runtime — the
// build script only needs `Cli::command()`, so the compiler correctly flags
// those items as unused *within this compilation unit*.
// `cli` was split from a single file into a folder (`src/cli/mod.rs` plus
// per-domain submodules) during a refactor. Point at the folder's mod.rs so
// this build script can keep using the same module path.
#[path = "src/cli/mod.rs"]
#[allow(dead_code)]
mod cli;

use clap::CommandFactory;
use clap_complete::Shell;
use std::path::{Path, PathBuf};

fn main() {
    // ── Rerun policy ──────────────────────────────────────────────────────────
    //
    // Cargo caches build-script output.  By default it reruns the script on
    // every build.  These two directives tell it to only rerun when the CLI
    // definition itself (or this script) changes — keeping incremental builds
    // fast for all other source edits.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/cli");

    // ── Output directories ────────────────────────────────────────────────────
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

    // man pages go into man/man1/ (the section subdirectory).
    // This matches the layout that `man(1)` expects when you point MANPATH at
    // the repo root, so you can test without installing system-wide:
    //
    //   MANPATH="$(pwd)/man" man g
    //   MANPATH="$(pwd)/man" man g-workspace-create
    //
    // For system installation, copy the whole man1/ directory:
    //   sudo cp man/man1/*.1 /usr/local/share/man/man1/
    let man_dir = root.join("man").join("man1");
    let completions_dir = root.join("completions");
    let docs_content_dir = root.join("docs/src/content/docs");

    std::fs::create_dir_all(&man_dir).expect("failed to create man/man1/");
    std::fs::create_dir_all(&completions_dir).expect("failed to create completions/");

    // ── Build the Clap Command object ─────────────────────────────────────────
    //
    // `CommandFactory::command()` is the trait method that clap's derive macro
    // generates for every `#[derive(Parser)]` struct.  We pin the name to "g"
    // here because the top-level struct intentionally omits `name = "…"` so
    // that the binary reads the real executable name at runtime — we need a
    // stable name for file naming and man-page section headers.
    let cmd = cli::Cli::command().name("g");

    // ── Man pages ─────────────────────────────────────────────────────────────
    //
    // `clap_mangen::Man` converts a Clap `Command` into ROFF source (the
    // format read by the `man` command).  We recurse into every subcommand so
    // each level gets its own page:
    //
    //   g.1                    → man g
    //   g-workspace.1          → man g-workspace
    //   g-workspace-create.1   → man g-workspace-create
    //   g-stack.1              → man g-stack
    //   … etc.
    generate_man_pages(&cmd, &man_dir).expect("man page generation failed");

    // ── Shell completions ─────────────────────────────────────────────────────
    //
    // `clap_complete::generate_to` writes a shell completion script for the
    // given shell into `completions/`.  The filename is determined by the shell
    // (e.g. `g.bash`, `_g` for zsh, `g.fish`, `g.elvish`).
    let mut cmd_completions = cmd.clone();
    for &shell in &[Shell::Bash, Shell::Zsh, Shell::Fish, Shell::Elvish] {
        clap_complete::generate_to(shell, &mut cmd_completions, "g", &completions_dir)
            .unwrap_or_else(|e| panic!("completion generation for {shell:?} failed: {e}"));
    }

    // ── Astro CLI reference ───────────────────────────────────────────────────
    //
    // `clap_markdown` serialises the entire command tree (commands, flags, and
    // their doc-comment descriptions) into GitHub-flavoured Markdown.  We
    // prepend Astro content-collection frontmatter so the file is picked up by
    // the docs site automatically.
    //
    // The docs/ directory may not exist in all build environments (e.g. a bare
    // `cargo install` from crates.io).  The guard keeps the build clean there.
    if docs_content_dir.exists() {
        let body = clap_markdown::help_markdown_command(&cmd);
        let content = format!(
            "\
---
title: CLI Reference
description: >-
  Complete reference for all g commands, subcommands, and flags.
  Auto-generated from src/cli/ — do not edit this file directly.
order: 99
---

<!-- This file is auto-generated by build.rs from src/cli/.         -->
<!-- Run `cargo build` to regenerate it after editing src/cli/*.rs. -->

{body}"
        );
        std::fs::write(docs_content_dir.join("cli-reference.md"), content)
            .expect("failed to write docs/src/content/docs/cli-reference.md");
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Recursively generate ROFF man pages for `cmd` and every subcommand.
///
/// The page for a subcommand is named by joining the parent name and the
/// subcommand name with a hyphen, so `g workspace create` becomes
/// `g-workspace-create.1`.  The auto-generated `help` subcommand and any
/// internal catch-all entries (names starting with `__`) are skipped.
fn generate_man_pages(
    cmd: &clap::Command,
    out_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let name = cmd.get_name().to_string();

    // Render this command into a ROFF buffer then persist it.
    let man = clap_mangen::Man::new(cmd.clone());
    let mut buf = Vec::<u8>::new();
    man.render(&mut buf)?;
    std::fs::write(out_dir.join(format!("{name}.1")), &buf)?;

    // Recurse into each subcommand, giving it the hyphen-joined full name.
    for sub in cmd.get_subcommands() {
        let sub_name = sub.get_name();
        // Skip the auto-generated `help` subcommand and clap-internal entries.
        if sub_name == "help" || sub_name.starts_with("__") || sub_name.is_empty() {
            continue;
        }
        // `Command::name()` requires `&'static str`.  We promote the
        // heap-allocated name string to `'static` via `Box::leak` — this is
        // intentional and harmless in a short-lived build script process.
        let full_name: &'static str = Box::leak(format!("{name}-{sub_name}").into_boxed_str());
        let renamed = sub.clone().name(full_name);
        generate_man_pages(&renamed, out_dir)?;
    }

    Ok(())
}
