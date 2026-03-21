---
title: Introduction
description: What g is, how it relates to git, and when to use it.
order: 0
---

`g` is a Rust CLI that **augments** Git: every standard `git` subcommand still works, while a curated set of commands (`log`, `status`, `diff`, `branch`, `show`, …) get richer terminal output, colors, and structure.

## Passthrough vs enhanced commands

**Passthrough** — anything that is not a built-in `g` subcommand is forwarded to `git` unchanged:

```bash
g pull origin main
g fetch --all --prune
g rebase -i HEAD~5
g tag -a v1.2.0 -m "release"
```

**Enhanced** — `g` intercepts these and adds formatting or UX, while still accepting normal git arguments where applicable:

```bash
g log --oneline -n 20
g status -sb
g diff HEAD~1 --stat
g branch -vv
g show HEAD
```

**Workflow commands** — stacks, workspaces, guided `commit`, `compare`, `config`:

```bash
g stack view
g workspace list
g compare main feature/x --stat
g commit              # interactive conventional commit (when no -m)
```

## What you get beyond plain Git

- **Workspaces** — parallel checkouts with `git worktree`, friendly names, and predictable sibling paths.
- **Stacks** — ordered branch chains for stacked PRs, with sync, push, and GitHub PR wiring.
- **Guided commits** — conventional-commit prompts with validation and previews.
- **Compare** — branch comparison with file stats, commit lists, or full diff through your configured tool.

Configuration lives in `~/.config/g/config.toml`. Workspace metadata and stack definitions are stored alongside that directory and are **not** part of your repository — they are local developer state keyed by repository path.

## Global flags (all commands)

```bash
g -C /path/to/repo status     # run as if started in that directory
g -c ui.log_limit=50 log      # one-off config override
g --dry-run stack sync        # print planned steps without executing
```

## Design philosophy

1. **Transparent passthrough** — `g pull`, `g rebase`, `g cherry-pick`, etc. forward to `git` unchanged.
2. **Beautiful defaults** — enhanced commands should read well in a modern terminal without extra flags.
3. **Opt-in power** — stacks and GitHub integration require tokens and explicit commands; nothing phones home by default.

## Animated Git flows

[Git flows](./git-flows/) embeds **inline figures** next to the explanations for stacks, worktrees, and `g stack sync`. [Stacks](./stacks/) and [Workspaces](./workspaces/) include the same style of figure where those topics are introduced.

## Next steps

- [Use cases](./use-cases/) — playbooks (stacks, worktrees, log/diff, team defaults).
- [Git flows](./git-flows/) — diagrams + how they map to Git.
- [Installation](./installation/) — install from crates.io or from this repository with Cargo.
- [Log & diff](./log-and-diff/) — enhanced history and patches.
- [Workspaces](./workspaces/) — worktrees without context switching.
- [Stacks](./stacks/) — stacked PR workflow.
