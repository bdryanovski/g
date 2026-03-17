# g — Version CLI

A beautiful, opinionated Git CLI built in Rust. `g` is a full drop-in replacement for the `git` command that adds:

- 🎨 **Beautiful colored output** — enhanced log, status, diff, branch, show
- 🏗️ **Stacked PRs** — create, sync, and publish layered pull requests to GitHub with a single command
- 🗂️ **Workspaces** — parallel branch checkouts via `git worktree`, no branch switching needed
- ✍️ **Guided commits** — interactive conventional commit builder with validation
- 🔍 **Branch comparison** — visual ahead/behind, file stat bars, commit lists
- 🔌 **Pluggable diff tools** — auto-detects `delta` / `diff-so-fancy`, or configure your own
- ⚙️ **Config-driven** — everything tweakable via `~/.config/g/config.toml`

---

## Install instructions

```bash
# From source (requires Rust)
git clone https://github.com/your-org/g
cd g
cargo install --path .

# Verify
g --version
```

Set `GITHUB_TOKEN` for PR features:

```bash
export GITHUB_TOKEN=ghp_your_token_here   # add to .zshrc / .bashrc
```

---

## Quick Start

```bash
# All git commands work transparently
g pull
g fetch --all
g rebase origin/main

# Enhanced versions of common commands
g log
g status
g diff
g branch
g show HEAD
```

---

## Commands

### `g log`

Beautiful colored commit log with graph, conventional commit type coloring, and ref decorations.

```
g log                  # last 30 commits (configurable)
g log -n 50
g log --all
g log --no-graph       # disable the graph
g log main..HEAD       # range
```

### `g status`

Enhanced status with icons, staged/unstaged/untracked sections, ahead/behind tracking info.

```
g status
```

### `g diff`

Auto-detects and pipes through `delta` or `diff-so-fancy` if available.

```
g diff
g diff HEAD~3
g diff main..feature-branch
```

### `g branch`

Rich branch table with hash, last commit subject, author, date, and upstream tracking.

```
g branch               # list all branches
g branch -b new-feat   # create (passes through to git)
g branch -d old-feat   # delete (passes through)
```

### `g show`

Beautiful commit header + diff.

```
g show
g show abc1234
```

---

## Guided Commits

```bash
g commit
```

Interactive step-by-step commit builder:

1. **Type** — pick from your configured conventional commit types (feat, fix, docs, …)
2. **Scope** — optional component/area
3. **Subject** — validated against max length
4. **Body** — explain _why_, not _what_
5. **Footer** — `BREAKING CHANGE:`, `Closes #123`, etc.

**Preview** is shown before confirming. Live character count warns you when the subject is too long.

```bash
g commit -a            # stage all + commit
g commit --amend       # amend last commit
g commit -m "feat: quick non-interactive"
```

---

## Workspaces

Workspaces are an abstraction on top of `git worktree`. Each workspace is a **separate directory** with its own checkout of a branch — no branch switching, no stashing, no context loss. You can have multiple branches checked out simultaneously.

```bash
# Create a workspace (creates sibling directory + new branch)
g workspace create feature-auth --description "Auth system"

# Create a workspace for an existing branch
g workspace create hotfix -b fix/login-bug

# List all worktree workspaces
g workspace list

# Open a subshell inside a workspace directory
g workspace switch feature-auth
# ... work on it, then Ctrl+D or `exit` to return

# Show info about the worktree you're currently in
g workspace status

# Rename (moves directory + repairs git tracking)
g workspace rename feature-auth auth-system

# Remove a workspace
g workspace delete feature-auth
g workspace delete feature-auth --force   # if worktree is dirty
```

Worktrees are created as **siblings** to your repo. If your repo lives at `~/proj/myapp`, a workspace named `feature-auth` creates a checkout at `~/proj/myapp--feature-auth`. The separator is configurable:

```toml
[workspace]
separator = "--"
```

---

## Stacked Pull Requests

Stacked PRs let you break large changes into a series of small, reviewable PRs that each build on the previous one. `g stack` manages the rebase chain and GitHub PR creation for you.

### Workflow

```bash
# 1. Start on main, create a stack
git checkout main
g stack new my-feature

# 2. Add the first layer
g stack add feature/auth-models

# Make your changes, commit...
g commit

# 3. Add another layer on top
g stack add feature/auth-api

# More changes, commit...
g commit

# 4. View the stack
g stack view
# Stack: my-feature
#
#   ├── ◯ main
#   │   │
#   ├── ◯ feature/auth-models
#   │   │
#   └── ◉ feature/auth-api  ← you are here

# 5. Push all branches
g stack push

# 6. Create GitHub PRs (each targeting the branch below)
g stack pr --open

# 7. If you amend feature/auth-models, sync the whole chain
g stack sync
```

### Stack Commands

```bash
g stack new <name>        # create a new stack at current branch
g stack add <branch>      # create and append a new branch to stack
g stack list              # list all stacks
g stack view              # tree view of current stack
g stack sync              # rebase each branch onto the one below
g stack push              # push all branches
g stack push --force      # force-push with lease
g stack pr                # create/update GitHub PRs
g stack pr --draft        # as draft PRs
g stack pr --open         # open PRs in browser after creating
g stack remove <branch>   # remove a branch from the stack (doesn't delete it)
g stack delete <name>     # delete the stack record
g stack delete <name> --branches  # also delete all git branches
```

---

## Branch Comparison

```bash
g compare                          # current branch vs main
g compare feature/foo              # main vs feature/foo
g compare main feature/foo         # explicit base and head
g compare --stat                   # file stat only
g compare --commits                # commits only
g compare --diff                   # full diff
```

---

## Configuration

Config lives at `~/.config/g/config.toml`. Generated automatically on first run.

```bash
g config          # show summary
g config --path   # print path
g config --edit   # open in $EDITOR
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
separator = "--"   # repo--workspace sibling directory naming

[aliases]
co = "checkout"
br = "branch"
st = "status"
lg = "log"
rb = "rebase"
sw = "switch"

[plugins]
discover = true    # loads g-* binaries from PATH
paths = []
```

---

## Diff Tools

`g diff` auto-detects the best available tool:

| Tool                                                       | Install                      |
| ---------------------------------------------------------- | ---------------------------- |
| [delta](https://github.com/dandavison/delta)               | `brew install git-delta`     |
| [diff-so-fancy](https://github.com/so-fancy/diff-so-fancy) | `brew install diff-so-fancy` |
| builtin                                                    | (always available)           |

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
g co main      # → git checkout main
g lg           # → git log --oneline (enhanced)
g undo         # → git reset --soft HEAD~1
```

---

## Plugins

Any binary named `g-<name>` in your `$PATH` becomes a `vcli <name>` command:

```bash
# Create ~/bin/g-deploy (executable)
#!/bin/bash
echo "deploying..."
```

```bash
g deploy   # runs vcli-deploy
```

Or specify explicit paths in config:

```toml
[plugins]
paths = ["/path/to/my-plugin", "~/scripts/g-release"]
```

---

## Environment Variables

| Variable       | Purpose                                              |
| -------------- | ---------------------------------------------------- |
| `GITHUB_TOKEN` | GitHub personal access token (preferred over config) |
| `EDITOR`       | Editor for `g config --edit` (default: `vim`)        |
| `NO_COLOR`     | Disable all color output                             |

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

~/.config/g/
├── config.toml       — Main config
├── workspaces.toml   — Workspace metadata (names, descriptions)
└── stacks.toml       — Stack store
```
