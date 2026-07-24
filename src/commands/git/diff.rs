//! `g diff` — builtin syntect-backed renderer with passthrough fallback.
//!
//! Three modes:
//! - **builtin** (default, `[diff].tool = "builtin"` or `"auto"`): in-process
//!   pipeline via [`crate::diff`].  When stdout is a TTY and not
//!   `--no-interactive`, opens the full-screen [`diff::render_tui`]; otherwise
//!   dumps inline ANSI via [`diff::render_inline`].
//! - **raw** (`--raw` flag or `[diff].tool = "raw"`): forward `git diff`
//!   output **untouched** to stdout — no rendering, no external pager, no
//!   color manipulation.  Same as running `git diff` directly.
//! - **external tool** (any other `[diff].tool` value treated as a path):
//!   pipe `git diff` stdout through an arbitrary executable.  Generic — no
//!   per-tool logic.
//!
//! The `--raw` flag, when supplied on the command line, always wins regardless
//! of `[diff].tool` so users can quickly drop out of the builtin render path
//! for scripting.
//!
//! When the TUI is active and the command is invoked from inside a git repo
//! (so `ctx.repo_id` is set), private review notes left via the `c` key are
//! saved to the local SQLite DB through [`crate::diff::reviews`].

use anyhow::{Context, Result};
use std::io::IsTerminal;
use std::process::{Command, Stdio};

use crate::commands::Ctx;
use crate::config;
use crate::diff;

use super::exec::{git_exe, passthrough};

/// Run diff in one of three modes (see the module docs).
///
/// `ctx` May carry a SQLite connection + `repo_id` so the TUI can persist
/// private review notes left via the `c` key.  When `ctx.repo_id` is `None`
/// (e.g. invoked from outside a git repo) the `c` key is a no-op and the TUI
/// stays a pure viewer.
pub fn enhanced_diff(ctx: &Ctx<'_>, extra_args: &[String]) -> Result<()> {
    let cfg = config::load().unwrap_or_default();

    // Strip `--raw` from the trailing args; it never reaches `git diff` itself.
    let mut user_args = Vec::with_capacity(extra_args.len());
    let mut raw_passthrough = false;
    for arg in extra_args {
        if arg == "--raw" {
            raw_passthrough = true;
        } else {
            user_args.push(arg.clone());
        }
    }

    // `--raw` flag on the command line takes precedence over everything.
    if raw_passthrough {
        return passthrough(&prepend_subcommand("diff", &user_args));
    }

    // SHA the diff was computed against, when one can be inferred.  Used to
    // anchor review notes to a commit so they survive future refactors via
    // git blame.  Best-effort — `None` for unstaged working-tree diffs.
    let commit_hash = infer_commit_hash(&user_args);

    match cfg.diff.tool.as_str() {
        // Persistent variant of `--raw`: forward git diff untouched.
        "raw" => passthrough(&prepend_subcommand("diff", &user_args)),
        // Default: our syntect-backed renderer (TUI if TTY, inline otherwise).
        "builtin" | "auto" | "" => {
            if !stdout_is_tty() || crate::ui::is_no_interactive() {
                builtin_inline(&user_args, &cfg)
            } else {
                builtin_tui(ctx, &user_args, &cfg, commit_hash.as_deref())
            }
        }
        // Any other value: treat as an external executable path and pipe
        // through it.  We don't apply per-tool logic; the executable decides
        // how to render what git emits.  If the named binary isn't on `$PATH`
        // or doesn't exist as a file, fall back to the builtin renderer so
        // the user still sees something useful.
        other => {
            if executable_available(other) {
                pipe_external(other, &user_args, &cfg)
            } else if !stdout_is_tty() || crate::ui::is_no_interactive() {
                builtin_inline(&user_args, &cfg)
            } else {
                builtin_tui(ctx, &user_args, &cfg, commit_hash.as_deref())
            }
        }
    }
}

/// Run the builtin inline-ANSI renderer over `git diff <args>`.
fn builtin_inline(args: &[String], cfg: &config::Config) -> Result<()> {
    let raw = git_diff_text(args, cfg)?;
    let mut files = diff::parse::parse(&raw);

    // For plain `g diff` (no args), also include untracked files
    if args.is_empty() {
        let untracked = get_untracked_diffs(cfg)?;
        files.extend(untracked);
    }

    if files.is_empty() {
        return Ok(());
    }
    diff::render_inline::render(&files);
    Ok(())
}

/// Open the builtin full-screen TUI over `git diff <args>`.
fn builtin_tui(
    ctx: &Ctx<'_>,
    args: &[String],
    cfg: &config::Config,
    commit_hash: Option<&str>,
) -> Result<()> {
    let raw = git_diff_text(args, cfg)?;
    let mut files = diff::parse::parse(&raw);

    // For plain `g diff` (no args), also include untracked files
    if args.is_empty() {
        let untracked = get_untracked_diffs(cfg)?;
        files.extend(untracked);
    }

    if files.is_empty() {
        return Ok(());
    }
    diff::render_tui::run(&files, ctx, commit_hash)
}

/// Pipe `git diff <args> <tool_args>` through `tool` as a generic pager.
///
/// All colour-mode decisions are left to `tool` and the user's git config —
/// we don't force `--color=always` for any specific binary.  If a user needs
/// colour codes fed into their tool they can add `--color=always` to
/// `[diff].tool_args` or set `color.diff = always` in their git config.
fn pipe_external(tool: &str, args: &[String], cfg: &config::Config) -> Result<()> {
    let mut git_args = vec!["diff".to_string()];
    git_args.extend(args.iter().cloned());
    git_args.extend(cfg.diff.tool_args.iter().cloned());
    let git_arg_refs: Vec<&str> = git_args.iter().map(String::as_str).collect();

    let output = Command::new(git_exe())
        .args(&git_arg_refs)
        .stdout(Stdio::piped())
        .spawn()?
        .stdout
        .context("no stdout from git diff")?;
    Command::new(tool)
        .args(&cfg.diff.tool_args)
        .stdin(output)
        .status()?;
    Ok(())
}

/// Capture `git diff` stdout as a string.
///
/// Adds the configured context-line count and `--no-color` so the parser
/// never has to strip ANSI sequences.
fn git_diff_text(args: &[String], cfg: &config::Config) -> Result<String> {
    let mut git_args = vec![
        "diff".to_string(),
        "--no-color".to_string(),
        format!("--unified={}", cfg.diff.context_lines),
    ];
    git_args.extend(args.iter().cloned());
    let arg_refs: Vec<&str> = git_args.iter().map(String::as_str).collect();

    let out = Command::new(git_exe())
        .args(&arg_refs)
        .output()
        .context("Failed to run git diff")?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if !stderr.is_empty() {
            anyhow::bail!("{stderr}");
        }
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// `["diff", …args…]`
fn prepend_subcommand(sub: &str, args: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len() + 1);
    out.push(sub.to_string());
    out.extend_from_slice(args);
    out
}

/// Get list of untracked files and generate diffs for them (as new files).
fn get_untracked_diffs(cfg: &config::Config) -> Result<Vec<diff::parse::FileDiff>> {
    // Get untracked files
    let out = Command::new(git_exe())
        .args(["ls-files", "--others", "--exclude-standard"])
        .output()
        .context("Failed to list untracked files")?;

    if !out.status.success() {
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let untracked: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();

    if untracked.is_empty() {
        return Ok(vec![]);
    }

    // Generate diff for untracked files using --no-index against /dev/null
    let mut files = Vec::new();
    for path in untracked {
        // Skip binary files and very large files
        if is_binary_file(path) || is_large_file(path) {
            // Add as binary/large file placeholder
            files.push(diff::parse::FileDiff {
                path: path.to_string(),
                old_path: None,
                status: diff::parse::Status::Added,
                stat: Some(diff::parse::Stat {
                    added: 0,
                    deleted: 0,
                }),
                hunks: vec![],
            });
            continue;
        }

        // Use git diff --no-index to diff against nothing
        let diff_out = Command::new(git_exe())
            .args([
                "diff",
                "--no-color",
                &format!("--unified={}", cfg.diff.context_lines),
                "--no-index",
                "/dev/null",
                path,
            ])
            .output();

        if let Ok(diff_out) = diff_out {
            let raw = String::from_utf8_lossy(&diff_out.stdout);
            let mut parsed = diff::parse::parse(&raw);
            // Fix up the path (git diff --no-index shows full paths)
            for f in &mut parsed {
                f.path = path.to_string();
                f.status = diff::parse::Status::Added;
            }
            files.extend(parsed);
        }
    }

    Ok(files)
}

/// Check if a file appears to be binary.
fn is_binary_file(path: &str) -> bool {
    use std::fs::File;
    use std::io::Read;

    let Ok(mut file) = File::open(path) else {
        return false;
    };

    let mut buffer = [0u8; 8192];
    let Ok(n) = file.read(&mut buffer) else {
        return false;
    };

    // Check for null bytes (common binary indicator)
    buffer[..n].contains(&0)
}

/// Check if a file is too large to diff (> 1MB).
fn is_large_file(path: &str) -> bool {
    std::fs::metadata(path)
        .map(|m| m.len() > 1_000_000)
        .unwrap_or(false)
}

/// Best-effort: figure out which commit a diff was computed against, so review
/// notes can be anchored to that commit and re-found via `git blame` later.
///
/// Returns the SHA as a hex `String`, or `None` when the diff covers the
/// unstaged working tree (no commit attribution available).
fn infer_commit_hash(args: &[String]) -> Option<String> {
    // First non-flag positional argument is treated as a rev specifier.  We
    // accept the loose `git diff [<commit>] [-- <path>]` form: a single
    // leading rev, or `<a>..<b>` / `<a>...<b>` ranges.
    let mut rev: Option<&str> = None;
    for arg in args {
        if arg == "--" {
            break;
        }
        if arg.starts_with('-') {
            continue;
        }
        // First positional that doesn't look like a path-only trailing arg.
        if rev.is_none() {
            rev = Some(arg);
        }
    }

    let target = match rev {
        Some(r) => r,
        None => {
            // `git diff --cached` compares index → HEAD; HEAD is the band.
            if args.iter().any(|a| a == "--cached" || a == "--staged") {
                "HEAD"
            } else {
                // Plain `git diff` with no revs = unstaged working-tree diff.
                return None;
            }
        }
    };

    let out = Command::new(git_exe())
        .args(["rev-parse", "--verify", target])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if sha.is_empty() {
        None
    } else {
        Some(sha)
    }
}

/// `true` when `name` is an executable on `$PATH` or an existing file path.
fn executable_available(name: &str) -> bool {
    if name.contains('/') || name.contains('\\') {
        return std::path::Path::new(name).exists();
    }
    which::which(name).is_ok()
}

/// `true` when stdout is connected to a terminal (vs. a pipe / file).
fn stdout_is_tty() -> bool {
    std::io::stdout().is_terminal()
}
