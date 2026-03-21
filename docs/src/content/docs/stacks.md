---
title: Stacks
description: Stacked branches, sync, push, and chained GitHub pull requests.
order: 5
---

**Stacks** model a linear chain of branches: each branch builds on the one below. That maps cleanly to **stacked PRs** on GitHub, where each PR targets the branch under it instead of always targeting `main`.

## Typical workflow

1. From your default branch, start a stack: `g stack new my-feature`
2. Add a layer: `g stack add feature/auth-models` — commit work with `g commit` or plain `git commit`
3. Add another layer: `g stack add feature/auth-api`
4. Inspect: `g stack view` or `g stack details`
5. After upstream changes, propagate: `g stack sync`
6. Publish: `g stack push` then `g stack pr` (optionally `--open` or `--draft`)

## Command reference

| Command | Purpose |
|--------|---------|
| `g stack new <name>` | Start a stack from the current branch |
| `g stack add <branch>` | Create and append a branch |
| `g stack list` | List stacks for this repo |
| `g stack view` | Tree view of current stack |
| `g stack details` | Commits / PR-oriented detail view |
| `g stack switch <name>` | Jump to top branch of a stack |
| `g stack sync` | Rebase each branch onto the one below |
| `g stack absorb` | Merge current branch into below and collapse |
| `g stack push` | Push all branches (`--force` with lease when needed) |
| `g stack pr` | Create or update chained PRs via GitHub API |
| `g stack remove <branch>` | Remove branch from stack metadata only |
| `g stack delete <name>` | Drop stack record (`--branches` to delete git branches too) |
| `g stack up` / `g stack down` | Reorder stack in the local list |

## Requirements

- **`GITHUB_TOKEN`** (or `[github].token` in config) for `g stack pr`
- Remote and repo detection must match what the GitHub module expects (see main README)

Stack order and PR numbers are persisted locally (e.g. `stacks.toml` under the config directory), keyed by repository.

## Dry run

Use the global **`--dry-run`** flag to preview git operations and side effects without mutating the repo or calling mutating APIs — useful before a large `sync` or `push`.
