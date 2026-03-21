---
title: Introduction
description: What g is, how it relates to git, and when to use it.
order: 0
---

`g` is a Rust CLI that **augments** Git: every standard `git` subcommand still works, while a curated set of commands (`log`, `status`, `diff`, `branch`, `show`, …) get richer terminal output, colors, and structure.

It also adds first-class workflows that are awkward with plain Git alone:

- **Workspaces** — parallel checkouts with `git worktree`, friendly names, and safe directory layout.
- **Stacks** — ordered branch chains for stacked PRs, with sync, push, and GitHub PR wiring.
- **Guided commits** — conventional-commit prompts with validation.
- **Compare** — visual branch diffs with stats, commits, or full diff through your tool of choice.

Configuration lives in `~/.config/g/config.toml`. Workspace metadata and stack definitions are stored alongside that directory and are **not** part of your repository — they are local developer state tied to the repo path.

## Design philosophy

1. **Transparent passthrough** — `g pull`, `g rebase`, `g cherry-pick`, etc. forward to `git` unchanged.
2. **Beautiful defaults** — enhanced commands should read well in a modern terminal without extra flags.
3. **Opt-in power** — stacks and GitHub integration require tokens and explicit commands; nothing phones home by default.

## Next steps

- [Use cases](./use-cases/) — playbooks (stacks, worktrees, log/diff, team defaults).
- [Installation](./installation/) — install from crates.io or from this repository with Cargo.
- [Log & diff](./log-and-diff/) — enhanced history and patches.
- [Workspaces](./workspaces/) — worktrees without context switching.
- [Stacks](./stacks/) — stacked PR workflow.
