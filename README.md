# vcli — Version CLI

A beautiful, opinionated Git CLI built in Rust. `vcli` is a full drop-in replacement for the `git` command that adds:

- 🎨 **Beautiful colored output** — enhanced log, status, diff, branch, show
- 🏗️  **Stacked PRs** — create, sync, and publish layered pull requests to GitHub with a single command  
- 🗂️  **Workspaces** — named snapshots of your branch + env files, switch instantly  
- ✍️  **Guided commits** — interactive conventional commit builder with validation  
- 🔍 **Branch comparison** — visual ahead/behind, file stat bars, commit lists  
- 🔌 **Pluggable diff tools** — auto-detects `delta` / `diff-so-fancy`, or configure your own  
- ⚙️  **Config-driven** — everything tweakable via `~/.config/vcli/config.toml`

---

## Install

```bash
# From source (requires Rust)
git clone https://github.com/your-org/vcli
cd vcli
cargo install --path .

# Verify
vcli --version
```

Set `GITHUB_TOKEN` for PR features:

```bash
export GITHUB_TOKEN=ghp_your_token_here   # add to .zshrc / .bashrc
```

---

## Quick Start

```bash
# All git commands work transparently
vcli pull
vcli fetch --all
vcli rebase origin/main

# Enhanced versions of common commands
vcli log
vcli status
vcli diff
vcli branch
vcli show HEAD
```

---

## Commands

### `vcli log`

Beautiful colored commit log with graph, conventional commit type coloring, and ref decorations.

```
vcli log                  # last 30 commits (configurable)
vcli log -n 50
vcli log --all
vcli log --no-graph       # disable the graph
vcli log main..HEAD       # range
```

### `vcli status`

Enhanced status with icons, staged/unstaged/untracked sections, ahead/behind tracking info.

```
vcli status
```

### `vcli diff`

Auto-detects and pipes through `delta` or `diff-so-fancy` if available.

```
vcli diff
vcli diff HEAD~3
vcli diff main..feature-branch
```

### `vcli branch`

Rich branch table with hash, last commit subject, author, date, and upstream tracking.

```
vcli branch               # list all branches
vcli branch -b new-feat   # create (passes through to git)
vcli branch -d old-feat   # delete (passes through)
```

### `vcli show`

Beautiful commit header + diff.

```
vcli show
vcli show abc1234
```

---

## Guided Commits

```bash
vcli commit
```

Interactive step-by-step commit builder:

1. **Type** — pick from your configured conventional commit types (feat, fix, docs, …)  
2. **Scope** — optional component/area  
3. **Subject** — validated against max length  
4. **Body** — explain *why*, not *what*  
5. **Footer** — `BREAKING CHANGE:`, `Closes #123`, etc.

**Preview** is shown before confirming. Live character count warns you when the subject is too long.

```bash
vcli commit -a            # stage all + commit
vcli commit --amend       # amend last commit
vcli commit -m "feat: quick non-interactive"
```

---

## Workspaces

Workspaces let you snapshot a branch + all your `.env` files so you can switch contexts cleanly.

```bash
vcli workspace create backend-work --description "API refactor"
vcli workspace create frontend-work

vcli workspace list       # see all workspaces
vcli workspace switch backend-work
vcli workspace status     # show current workspace info
vcli workspace rename backend-work api-refactor
vcli workspace delete frontend-work
```

**What gets copied:** Any file matching `workspace.copy_patterns` in config:

```toml
[workspace]
copy_patterns = [".env", ".env.local", ".env.*.local"]
auto_stash = true
```

Switching automatically stashes uncommitted work, checks out the workspace branch, and restores env files.

---

## Stacked Pull Requests

Stacked PRs let you break large changes into a series of small, reviewable PRs that each build on the previous one. `vcli stack` manages the rebase chain and GitHub PR creation for you.

### Workflow

```bash
# 1. Start on main, create a stack
git checkout main
vcli stack new my-feature

# 2. Add the first layer
vcli stack add feature/auth-models

# Make your changes, commit...
vcli commit

# 3. Add another layer on top
vcli stack add feature/auth-api

# More changes, commit...
vcli commit

# 4. View the stack
vcli stack view
# Stack: my-feature
#
#   ├── ◯ main
#   │   │
#   ├── ◯ feature/auth-models
#   │   │
#   └── ◉ feature/auth-api  ← you are here

# 5. Push all branches
vcli stack push

# 6. Create GitHub PRs (each targeting the branch below)
vcli stack pr --open

# 7. If you amend feature/auth-models, sync the whole chain
vcli stack sync
```

### Stack Commands

```bash
vcli stack new <name>        # create a new stack at current branch
vcli stack add <branch>      # create and append a new branch to stack
vcli stack list              # list all stacks
vcli stack view              # tree view of current stack
vcli stack sync              # rebase each branch onto the one below
vcli stack push              # push all branches
vcli stack push --force      # force-push with lease
vcli stack pr                # create/update GitHub PRs
vcli stack pr --draft        # as draft PRs
vcli stack pr --open         # open PRs in browser after creating
vcli stack remove <branch>   # remove a branch from the stack (doesn't delete it)
vcli stack delete <name>     # delete the stack record
vcli stack delete <name> --branches  # also delete all git branches
```

---

## Branch Comparison

```bash
vcli compare                          # current branch vs main
vcli compare feature/foo              # main vs feature/foo
vcli compare main feature/foo         # explicit base and head
vcli compare --stat                   # file stat only
vcli compare --commits                # commits only
vcli compare --diff                   # full diff
```

---

## Configuration

Config lives at `~/.config/vcli/config.toml`. Generated automatically on first run.

```bash
vcli config          # show summary
vcli config --path   # print path
vcli config --edit   # open in $EDITOR
```

### Key Options

```toml
[general]
default_branch = "main"
auto_fetch = false
# pager = "less"           # override pager
# git_path = "/usr/bin/git"

[ui]
colors = true
icons = true
date_format = "relative"   # "relative" | "short" | "iso" | "rfc"
log_limit = 30
show_graph = true

[commit]
types = ["feat", "fix", "docs", "refactor", "perf", "test", "build", "ci", "chore", "revert"]
require_scope = false
require_body = false
max_subject_length = 72
gpg_sign = false

[diff]
tool = "auto"    # auto-detects delta/diff-so-fancy
# tool = "delta"
# tool = "diff-so-fancy"
# tool = "vimdiff"
# tool = "/path/to/my-diff"

[github]
# token = "..."             # prefer GITHUB_TOKEN env var
default_reviewers = ["alice", "bob"]
default_labels = ["needs-review"]

[workspace]
copy_patterns = [".env", ".env.local", ".env.*.local"]
auto_stash = true

[aliases]
co = "checkout"
br = "branch"
st = "status"
lg = "log"
rb = "rebase"
sw = "switch"

[plugins]
discover = true    # loads vcli-* binaries from PATH
paths = []
```

---

## Diff Tools

`vcli diff` auto-detects the best available tool:

| Tool | Install |
|------|---------|
| [delta](https://github.com/dandavison/delta) | `brew install git-delta` |
| [diff-so-fancy](https://github.com/so-fancy/diff-so-fancy) | `brew install diff-so-fancy` |
| builtin | (always available) |

Override in config: `diff.tool = "delta"` or point to any binary.

---

## Aliases

Aliases in `[aliases]` expand to full git commands transparently:

```toml
[aliases]
co = "checkout"
lg = "log --oneline"
undo = "reset --soft HEAD~1"
```

```bash
vcli co main      # → git checkout main
vcli lg           # → git log --oneline (enhanced)
vcli undo         # → git reset --soft HEAD~1
```

---

## Plugins

Any binary named `vcli-<name>` in your `$PATH` becomes a `vcli <name>` command:

```bash
# Create ~/bin/vcli-deploy (executable)
#!/bin/bash
echo "deploying..."
```

```bash
vcli deploy   # runs vcli-deploy
```

Or specify explicit paths in config:

```toml
[plugins]
paths = ["/path/to/my-plugin", "~/scripts/vcli-release"]
```

---

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `GITHUB_TOKEN` | GitHub personal access token (preferred over config) |
| `EDITOR` | Editor for `vcli config --edit` (default: `vim`) |
| `NO_COLOR` | Disable all color output |

---

## Building

```bash
# Debug build
cargo build

# Release build (optimized + stripped)
cargo build --release

# Run directly
cargo run -- log
cargo run -- status
cargo run -- workspace list
```

---

## Project Structure

```
src/
├── main.rs           — Entry point, command dispatch
├── cli.rs            — Clap CLI definitions
├── config/
│   └── mod.rs        — Config load/save, defaults, schema
├── ui/
│   └── mod.rs        — Colors, spinners, tables, formatters
├── commands/
│   ├── mod.rs
│   ├── git.rs        — Passthrough + enhanced log/status/diff/branch/show
│   ├── commit.rs     — Interactive guided commit
│   ├── compare.rs    — Branch comparison
│   ├── workspace.rs  — Workspace management
│   └── stack.rs      — Stacked PR management
└── github/
    └── mod.rs        — GitHub API client (PRs, repo detection)

~/.config/vcli/
├── config.toml       — Main config
├── workspaces.toml   — Workspace store
├── stacks.toml       — Stack store
└── workspace_files/  — Stored env file snapshots
    └── <workspace>/
        └── .env
```
