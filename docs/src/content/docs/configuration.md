---
title: Configuration
description: config.toml structure, aliases, plugins, and environment variables.
order: 7
---

Configuration is read from **`~/.config/g/config.toml`**. It is created with defaults on first use.

## Inspect and edit

```bash
g config              # summary + key paths
g config log_limit    # fuzzy search for a key (example)
g config --path       # print config file path only
g config --edit       # open in $EDITOR (default: vim if unset)
```

## Full example

```toml
[general]
default_branch = "main"
auto_fetch = false
# pager = "less"
# git_path = "/usr/bin/git"

[ui]
colors = true
icons = true
date_format = "relative"   # relative | short | iso | rfc
log_limit = 30
show_graph = true

[commit]
types = ["feat", "fix", "docs", "refactor", "perf", "test", "build", "ci", "chore", "revert"]
require_scope = false
require_body = false
max_subject_length = 72
gpg_sign = false

[diff]
tool = "auto"

[github]
# token = "…"              # prefer GITHUB_TOKEN in the environment
default_reviewers = ["alice"]
default_labels = ["needs-review"]

[workspace]
separator = "--"

[aliases]
co = "checkout"
br = "branch"
st = "status"
lg = "log"

[plugins]
discover = true
paths = []
```

## Section reference

### `[general]`

| Key | Meaning |
|-----|---------|
| `default_branch` | Used when comparing or inferring base branch |
| `auto_fetch` | Optional automatic fetch behavior (if implemented for a command) |
| `git_path` | Override path to `git` binary |
| `pager` | Pager for long output |

### `[ui]`

Controls enhanced **log**, **status**, **branch**, etc.

### `[commit]`

Drives **`g commit`** interactive flow: allowed types, subject length, GPG.

### `[diff]`

`tool = "auto"` tries **delta**, then **diff-so-fancy**, then falls back to built-in diff.

### `[github]`

Defaults for **`g stack pr`** (labels, reviewers). **Token:** use `GITHUB_TOKEN` when possible.

### `[workspace]`

`separator` is inserted between repo folder name and workspace name for default paths.

### `[aliases]`

Shorthand before passthrough:

```bash
g co main        # expands then runs through git
g st -sb
```

### `[plugins]`

Discover `g-*` executables on `PATH`, or list explicit binary paths.

## Environment variables

| Variable | Role |
|----------|------|
| `GITHUB_TOKEN` | GitHub API for `g stack pr` |
| `EDITOR` | `g config --edit` |
| `NO_COLOR` | Disable ANSI color |

## CLI overrides

Override any supported key for **one** invocation:

```bash
g -c ui.log_limit=100 log
g -c diff.tool=delta diff
```

## Related docs

- [Log & diff](./log-and-diff/) — what `[ui]` and `[diff]` affect
- [Workspaces](./workspaces/) — `[workspace]`
- [Stacks](./stacks/) — `[github]`
