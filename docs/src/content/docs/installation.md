---
title: Installation
description: Install g with Cargo from crates.io or from source, plus GitHub token setup.
order: 2
---

## Requirements

- **Rust toolchain** (stable) with `cargo` on your `PATH`
- **Git** installed and available as `git` (or set `general.git_path` in `~/.config/g/config.toml`)

Check versions:

```bash
rustc --version
cargo --version
git --version
```

## Install from crates.io

When the crate is published:

```bash
cargo install g
```

Pin a version for reproducible environments:

```bash
cargo install g --version 0.1.0
```

Verify:

```bash
which g
g --version
```

> **Note:** The package name in this repository is `g` (see the root `Cargo.toml`). If the crates.io name differs when you publish, substitute that name in `cargo install`.

## Install from this repository

The Rust project lives at the **repository root**; `docs/` is only the website.

```bash
git clone https://github.com/YOUR_ORG/vcli.git
cd vcli
cargo install --path .
```

Release-optimized binary:

```bash
cargo install --path . --locked
```

The binary is placed in `~/.cargo/bin` by default. Ensure it is on your `PATH`:

```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

## Update

```bash
cargo install g --force
# or from a git checkout:
cd vcli && git pull && cargo install --path . --force
```

## GitHub token (stacks & PRs)

Fine-grained or classic PAT with repo scope (as appropriate for your org):

```bash
export GITHUB_TOKEN=ghp_your_token_here
```

Persist in shell config:

```bash
echo 'export GITHUB_TOKEN=ghp_…' >> ~/.zshrc
```

Optional config file (prefer env for CI and shared machines):

```toml
# ~/.config/g/config.toml
[github]
# token = "ghp_…"
default_labels = ["needs-review"]
```

## Optional: shell alias

```bash
alias gg='g'
```

Aliasing **`git` itself** to `g` can break scripts that expect stock Git; prefer a dedicated command name.

## Verify enhanced commands

```bash
g log -n 3
g status
g diff --stat
```

## Shell completions

Completion generation is not built into the CLI yet. Until it ships, many users rely on **git** completions for passthrough commands or hand-written completions for `g`.

## Next

- [Introduction](./introduction/)
- [Configuration](./configuration/)
