//! Test-only fixtures for command-layer integration tests.
//!
//! Every command test wants the same setup: a real git repository on disk
//! (so `gitcmd::repo_root()` and friends work), an isolated `HOME` so
//! `config::config_dir()` doesn't read the developer's real `~/.config/g`,
//! and an in-memory SQLite [`Connection`] with migrations applied (so storage
//! calls work without touching disk).
//!
//! [`TestRepo`] bundles all of that.  A test reads like:
//!
//! ```ignore
//! let repo = TestRepo::new();          // tmp git repo + tmp HOME + in-mem db
//! let ctx = repo.ctx();
//! stack::new::run(&ctx, "my-stack").unwrap();
//! assert_eq!(repo.list_stacks(), vec!["my-stack"]);
//! ```
//!
//! ## Process-global state
//!
//! Two pieces of state are process-global and need synchronisation:
//!
//! - **`HOME` env var** — set once per test binary via [`init_test_home`]
//!   ([`OnceLock`]-backed) so every test in the same binary sees the same
//!   isolated home directory.
//! - **Current working directory** — `git rev-parse --show-toplevel` always
//!   runs in cwd, so [`TestRepo::new`] takes the [`CWD_LOCK`] mutex for the
//!   lifetime of the test, guaranteeing tests are serialised even if cargo
//!   runs them in parallel.

// The parent declaration in `commands/mod.rs` is already `#[cfg(test)] mod
// test_support;`, so the whole file is gated on `cfg(test)` from above — no
// inner `#![cfg(test)]` needed.
#![allow(dead_code)] // helpers grow over time; not every test uses every method

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

use rusqlite::Connection;
use tempfile::TempDir;

use crate::commands::Ctx;
use crate::storage::migrations;

/// Process-wide lock serialising tests that change the current working
/// directory.  Acquired by [`TestRepo::new`] and released on drop.
static CWD_LOCK: Mutex<()> = Mutex::new(());

/// Process-wide isolated HOME directory.  Created on first use and reused for
/// every test in the same binary — `std::env::set_var` is global state, so
/// "set it once" is the only safe option.
static TEST_HOME: OnceLock<TempDir> = OnceLock::new();

/// Ensure `HOME` points at an isolated TempDir for the lifetime of the test
/// process, so `config::config_dir()` and `themes_dir()` never resolve to the
/// developer's real `~/.config/g`.
fn init_test_home() {
    TEST_HOME.get_or_init(|| {
        let dir = TempDir::new().expect("create test HOME tempdir");
        // SAFETY: setting `HOME` is unsafe in recent Rust because it is a
        // process-wide mutation.  We only run this inside the OnceLock
        // initialiser, so it happens exactly once per process before any test
        // body observes `HOME`.
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        dir
    });
}

/// A fully-configured test environment: a real git repo on disk, an isolated
/// HOME, an in-memory SQLite database with migrations applied, and the
/// matching [`Ctx`].
pub(crate) struct TestRepo {
    /// Held to keep the lock until the test ends; never read.
    _cwd_guard: MutexGuard<'static, ()>,
    /// Held to keep the tempdir alive until the test ends.
    _dir: TempDir,
    /// Absolute path to the git repository root.
    pub root: PathBuf,
    /// In-memory database connection.
    pub conn: Connection,
}

impl TestRepo {
    /// Create a fresh git repo + in-memory DB, change cwd into the repo, and
    /// return the harness.
    pub fn new() -> Self {
        init_test_home();
        let cwd_guard = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let dir = TempDir::new().expect("create test repo tempdir");
        // Canonicalise so later `git rev-parse --show-toplevel` comparisons
        // match exactly on macOS (where `/var` is a symlink to `/private/var`).
        let root = std::fs::canonicalize(dir.path()).expect("canonicalise tempdir");

        // Initialise a real git repo with a deterministic identity and an
        // initial commit so `current_branch()` always returns "main".
        run_git(&root, &["init", "--quiet", "--initial-branch=main"]);
        run_git(&root, &["config", "user.email", "test@example.com"]);
        run_git(&root, &["config", "user.name", "Test"]);
        std::fs::write(root.join("README.md"), "test\n").expect("write README");
        run_git(&root, &["add", "."]);
        run_git(&root, &["commit", "--quiet", "-m", "initial"]);

        // Move into the repo so commands' git calls resolve to it.
        std::env::set_current_dir(&root).expect("cd into test repo");

        // Build the in-memory database and apply migrations.
        let conn = Connection::open_in_memory().expect("open in-memory db");
        migrations::run(&conn).expect("apply migrations");

        Self {
            _cwd_guard: cwd_guard,
            _dir: dir,
            root,
            conn,
        }
    }

    /// Return a `Ctx` borrowing this harness's database connection.
    pub fn ctx(&self) -> Ctx<'_> {
        Ctx::new(&self.conn)
    }

    /// Run a git command inside the test repo, returning trimmed stdout.
    /// Panics on non-zero exit so failures show up loudly in tests.
    pub fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .unwrap_or_else(|e| panic!("git {args:?} failed to spawn: {e}"));
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// Stage `path` with `content` and commit with the given message.
    pub fn commit(&self, path: &str, content: &str, message: &str) {
        std::fs::write(self.root.join(path), content).expect("write file");
        self.git(&["add", path]);
        self.git(&["commit", "--quiet", "-m", message]);
    }

    /// Create a new branch from HEAD and check it out.
    pub fn branch(&self, name: &str) {
        self.git(&["checkout", "--quiet", "-b", name]);
    }

    /// Check out an existing branch.
    pub fn checkout(&self, name: &str) {
        self.git(&["checkout", "--quiet", name]);
    }

    /// Return the current branch name (`git rev-parse --abbrev-ref HEAD`).
    pub fn current_branch(&self) -> String {
        self.git(&["rev-parse", "--abbrev-ref", "HEAD"])
    }
}

/// Run a git command in `cwd`, panicking on non-zero exit.
fn run_git(cwd: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|e| panic!("git {args:?}: spawn failed: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
