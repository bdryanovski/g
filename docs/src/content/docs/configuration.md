---
title: Configuration
description: config.toml structure, aliases, plugins, and environment variables.
order: 6
---

Configuration is read from **`~/.config/g/config.toml`**. It is created with defaults on first use.

## Inspect and edit

```bash
g config           # summary
g config <key>     # look up a value
g config --path    # print file path
g config --edit    # open in $EDITOR
```

## Notable sections

```toml
[general]
default_branch = "main"
auto_fetch = false
# git_path = "/usr/bin/git"

[ui]
colors = true
icons = true
date_format = "relative"
log_limit = 30
show_graph = true

[commit]
types = ["feat", "fix", "docs", "refactor", "perf", "test", "build", "ci", "chore", "revert"]
max_subject_length = 72

[diff]
tool = "auto"

[github]
# token via GITHUB_TOKEN preferred
default_reviewers = []
default_labels = []

[workspace]
separator = "--"

[aliases]
co = "checkout"
st = "status"

[plugins]
discover = true
paths = []
```

## Aliases

Entries under `[aliases]` expand **before** passthrough: `g co main` becomes the configured git invocation (e.g. `git checkout main`).

## Plugins

Executables named `g-<name>` on your `PATH` are exposed as `g <name>` when `plugins.discover` is true. You can also list explicit paths.

## Environment variables

| Variable | Role |
|----------|------|
| `GITHUB_TOKEN` | GitHub API for stack PR features |
| `EDITOR` | Used by `g config --edit` |
| `NO_COLOR` | Disable colored output |

## Overrides

Use **`-c key=value`** on the CLI to override config for a single invocation (see `g --help`).
