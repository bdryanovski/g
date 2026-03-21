---
title: Log & diff
description: Enhanced history, graph, and diff tooling with delta and diff-so-fancy.
order: 2
---

## Enhanced log

`g log` formats history for readability: graph, decorations, and conventional-commit styling (subject lines are easier to scan than raw `git log` defaults).

Common invocations:

```bash
g log              # default limit from config (e.g. last 30 commits)
g log -n 50
g log --all
g log --no-graph   # disable ASCII graph
g log main..HEAD   # range
```

Options after `g log` are forwarded to `git log`, so your existing muscle memory still applies.

### Configuration

In `~/.config/g/config.toml`, the `[ui]` section controls defaults such as `log_limit` and `show_graph`. Tune date formats and colors there for a consistent look across commands.

## Enhanced diff

`g diff` runs a normal Git diff but **pipes through an external tool** when configured or auto-detected:

- [delta](https://github.com/dandavison/delta)
- [diff-so-fancy](https://github.com/so-fancy/diff-so-fancy)
- or a custom executable (including `vimdiff`)

Examples:

```bash
g diff
g diff HEAD~3
g diff main..feature-branch
```

### Configure a diff tool

```toml
[diff]
tool = "auto"   # default: pick best available
# tool = "delta"
# tool = "diff-so-fancy"
# tool = "vimdiff"
# tool = "/path/to/my-diff"
```

Install candidates (macOS Homebrew examples):

```bash
brew install git-delta
brew install diff-so-fancy
```

## Related commands

- **`g show`** — commit header plus patch, styled like `g diff`.
- **`g status`** — icon-rich working tree summary with tracking hints.
- **`g branch`** — table of branches with last commit metadata.
- **`g compare`** — compare two branches with `--stat`, `--commits`, or `--diff` modes.
