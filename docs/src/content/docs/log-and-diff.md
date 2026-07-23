---
title: Log & diff
description: Enhanced history, graph, and diff tooling — builtin viewer or any external pager.
order: 3
---

## Enhanced log

`g log` formats history for readability: **graph**, **decorations**, and **conventional-commit** styling so subjects are easier to scan than raw `git log` defaults.

### Everyday commands

```bash
g log                        # default limit from config (e.g. 30 commits)
g log -n 50
g log --oneline -n 20
g log --all
g log --no-graph             # disable ASCII graph
g log main..HEAD             # what’s on current branch since it diverged from main
g log -p -1                  # last commit with patch (still enhanced where applicable)
```

Everything after `g log` is passed through to **`git log`**, so flags you already know keep working.

### Read a feature branch before merge

```bash
g log origin/main..HEAD
g log --stat origin/main..HEAD
```

### Configuration (`~/.config/g/config.toml`)

```toml
[ui]
log_limit = 30
show_graph = true
date_format = "relative"   # relative | short | iso | rfc
colors = true
icons = true
```

## Enhanced diff

`g diff` renders unified diffs with the builtin viewer by default:
syntax-highlighted stack / split / side-by-side TUI when stdout is a TTY,
otherwise inline ANSI.  Three modes are available:

- **`auto` / `builtin`** (default) — builtin renderer
- **`raw`** — forwards `git diff` output untouched (no rendering)
- **`/path/to/executable`** — pipes `git diff` stdout through any external pager
  (any executable on `$PATH` or an absolute path); falls back to the builtin
  renderer if the binary isn't found

### Examples

```bash
g diff
g diff --staged
g diff HEAD~3
g diff main...feature-branch    # three-dot: merge-base comparison
g diff main feature-branch -- path/to/file.rs
g diff --raw                    # passthrough for this single invocation
```

### Configure the diff mode

```toml
[diff]
tool = "auto"               # builtin (TUI if TTY, inline ANSI otherwise)
# tool = "builtin"          # same as "auto"
# tool = "raw"               # forward `git diff` untouched
# tool = "/path/to/my-pager" # any executable on $PATH or absolute path

# Optional:
# tool_args = ["--dark"]      # extra args passed to the external tool
# context_lines = 3           # unified diff context
# layout = "auto"             # auto | stack | split | side (TUI only)
# line_numbers = true
# wrap_lines = false
```

## Related commands

### `g show`

Single commit: header + patch, same diff tooling as `g diff`.

```bash
g show
g show abc1234
g show HEAD~2 --stat
```

### `g status`

Working tree with icons, grouped sections, and tracking hints.

```bash
g status
g status -sb
```

### `g branch`

Table of branches with last commit, author, date, upstream.

```bash
g branch
g branch -vv
g branch squash              # compact all commits on the branch (merge-base vs upstream / mainline)
g branch squash -m "feat: …" --base origin/main
```

`g branch squash` is for a **single** branch and does not restack a registered stack; for that, use `g stack squash` on the [Stacks](./stacks/) page.

### `g compare`

Compare two branches without merging—pick a view mode:

```bash
g compare                          # current vs default branch
g compare main feature/foo
g compare --stat                   # file-level bars / counts
g compare --commits                # commit subjects only
g compare --diff                   # full diff through configured tool
```

## Troubleshooting

- **No colors** — check `NO_COLOR` env; set `[ui] colors = true` in config.
- **Plain diff** — `[diff].tool` points at an executable that isn't on `$PATH`; set `tool = "auto"` or install the binary you pointed at.
- **Pager** — if output is truncated, configure `general.pager` in config (see main README).
