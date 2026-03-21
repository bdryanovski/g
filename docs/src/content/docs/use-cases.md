---
title: Use cases
description: Real scenarios where g shines—map problems to commands and docs.
order: 1
---

These playbooks mirror how teams actually work. Each links forward to deeper pages; mix and match as your workflow evolves.

## Ship a feature as stacked PRs

**Problem:** One big branch is hard to review; you want small PRs that depend on each other.

1. From your default branch: `g stack new auth-overhaul`
2. Layer branches: `g stack add feat/auth-models`, then commit, then `g stack add feat/auth-api`
3. Keep the chain honest after upstream moves: `g stack sync`
4. Publish: `g stack push` then `g stack pr --open`

**Why g:** Local stack metadata + GitHub API sets each PR’s base to the branch below. See [Stacks](./stacks/).

## Work on two branches without stashing

**Problem:** You’re deep in a feature but need to fix `main` or review a coworker’s branch.

1. `g workspace create hotfix-login -b fix/oauth` or create a new branch from current context
2. `g workspace list` to see paths and branches
3. `g workspace switch hotfix-login` for a subshell in that checkout
4. Remove when done: `g workspace delete hotfix-login`

**Why g:** Named worktrees as sibling directories—no checkout thrash. See [Workspaces](./workspaces/).

## Make `git log` scannable in reviews

**Problem:** Default `git log` is dense; you want graph + type coloring for conventional commits.

- `g log` for the default enhanced view (limit and graph from config)
- `g log main..HEAD` for what you’re about to merge
- Pair with `g compare --commits` for branch deltas

**Why g:** Same arguments as `git log`, prettier output. See [Log & diff](./log-and-diff/).

## Standardize diff review for the team

**Problem:** Everyone uses a different diff style in review.

- Install [delta](https://github.com/dandavison/delta) or [diff-so-fancy](https://github.com/so-fancy/diff-so-fancy)
- Set `diff.tool = "auto"` (default) or pin a tool in `~/.config/g/config.toml`
- Use `g diff`, `g show`, and `g compare --diff` for consistent piping

**Why g:** One config, all enhanced diff entry points. See [Log & diff](./log-and-diff/) and [Configuration](./configuration/).

## Next

- [Installation](./installation/) — Cargo install paths
- [Introduction](./introduction/) — mental model and philosophy
