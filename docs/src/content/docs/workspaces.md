---
title: Workspaces
description: Git worktrees with friendly names, sibling directories, and metadata.
order: 4
---

Workspaces wrap **`git worktree`**: each workspace is a **separate directory** with its own checkout. You keep multiple branches checked out at once without stashing or constantly switching branches.

## Create

```bash
# New branch named after the workspace (default)
g workspace create feature-auth --description "Auth system"

# Track an existing branch
g workspace create hotfix -b fix/login-bug
```

Worktrees are created as **siblings** of the main repo. If the repo is `~/proj/myapp`, a workspace `feature-auth` typically becomes `~/proj/myapp--feature-auth`. The separator is configurable.

## List & inspect

```bash
g workspace list
g workspace status
```

## Switch context

```bash
g workspace switch feature-auth
# subshell opens in that directory; exit when done
```

## Rename & remove

```bash
g workspace rename feature-auth auth-system
g workspace delete auth-system
g workspace delete auth-system --force   # if dirty
```

## Configuration

```toml
[workspace]
separator = "--"
```

Metadata (names, descriptions) is stored under `~/.config/g/` — it augments what Git already knows from `git worktree list`.
